# cargo --all-features が Linux で panic する

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-build-all-features-panic
- Polished: 2026-07-21

## 目的

Linux で `cargo clippy --all-features` や `cargo test --all-features` を実行すると、`default-v4l2` と `default-pipewire` の両方が有効化され、`build.rs:65-67` の `enable_default_count >= 2` チェックで panic する。cargo の `--all-features` は開発者・CI ツール（cargo-hack、cargo-nextest 等）が普通に使うフラグであり、これが壊れているのは開発体験を損なう。修正する。

## 優先度根拠

- Medium。ランタイムのバグではなくビルド時の問題だが、開発者体験と CI ツール互換性を損なう
- `cargo clippy --all-features` は IDE 統合や lint ツールが自動的に使うことがある
- `/review-code` の重要指摘として確認

## 現状

`Cargo.toml` の features:

```toml
default = ["default-avf", "default-mf", "default-v4l2"]
default-pipewire = ["pipewire"]
default-v4l2 = ["v4l2"]
```

`build.rs:44-51`:

```rust
if env::var("CARGO_FEATURE_DEFAULT_V4L2").is_ok() {
    println!("cargo::rustc-cfg=enable_default_v4l2");
    enable_default_count += 1;
}
if env::var("CARGO_FEATURE_DEFAULT_PIPEWIRE").is_ok() {
    println!("cargo::rustc-cfg=enable_default_pipewire");
    enable_default_count += 1;
}
```

`build.rs:65-67`:

```rust
if enable_default_count >= 2 {
    panic!("Multiple default backends selected. Enable exactly one default-* feature.");
}
```

`--all-features` は全 feature を有効化するため、Linux で `default-v4l2` と `default-pipewire` の両方が有効になり panic する。macOS / Windows では `default-pipewire` / `default-v4l2` の環境変数を読み取るコードが build.rs の `match target_os` の `"linux"` アーム内にあり、macOS / Windows ではそのアームに入らないため検査自体が実行されず、発生しない。

## 設計方針

`--all-features` 時の挙動を「panic ではなく、最初の default を優先」に変更する。

採用案: build.rs の構造を変更し、default 系 feature の検出と `cargo::rustc-cfg` の発行を分離する。現状は :44-51 で検出と同時に cfg を発行し、:65 で判定しているが、`cargo::rustc-cfg` は一度発行すると取り消せないため、先に全 default 系 feature を検出してカウントし、最後に優先順位で 1 つだけ cfg を発行する。

具体的には:
- :44-51（Linux ブランチ内の default 系検出 + cfg 発行）を以下のコードに置き換える
- :65-67（panic チェック）を削除する
- :23 の `let mut enable_default_count = 0;`、macOS ブランチ :31 の `enable_default_count += 1;`、Windows ブランチ :59 の `enable_default_count += 1;` をすべて削除する（panic チェック削除後は dead code になり clippy で警告が出るため）

```rust
// :44-51 を以下に置き換え（Linux ブランチ内）
let has_default_v4l2 = env::var("CARGO_FEATURE_DEFAULT_V4L2").is_ok();
let has_default_pipewire = env::var("CARGO_FEATURE_DEFAULT_PIPEWIRE").is_ok();

// 両方有効の場合は警告を出し、優先順位で 1 つだけ cfg を発行する。
// 両方の cfg を同時に発行すると Rust 側（device.rs, capture.rs）で
// 同一関数内に 2 つの実装ブロックが存在することになりコンパイルエラーになるため、
// 必ず 1 つだけ発行する。
if has_default_v4l2 && has_default_pipewire {
    println!("cargo::warning=Multiple default backends selected. Using default-v4l2.");
}
if has_default_v4l2 {
    println!("cargo::rustc-cfg=enable_default_v4l2");
} else if has_default_pipewire {
    println!("cargo::rustc-cfg=enable_default_pipewire");
}
// default 系 feature なし（--no-default-features 等）の場合はどちらの分岐にも入らず、
// cfg を発行しない（既存挙動と同一）
```

優先順位は `default-v4l2` > `default-pipewire`。根拠: V4L2 は Linux の標準的なビデオキャプチャ API であり、PipeWire は補完的なバックエンド。また `Cargo.toml:53` の `default = ["default-avf", "default-mf", "default-v4l2"]` に `default-v4l2` が含まれ `default-pipewire` は含まれないという設計意図とも整合する。

警告は `cargo::warning=` を使用する（cargo がユーザーに表示する build script 規約）。

### 採用しない案

- `--all-features` 検出時に default 系 feature を全て無視する: build.rs から `--all-features` を直接検出する手段は存在しない。Cargo は build script に起動フラグを伝搬しない。技術的に不可能
- Cargo.toml の feature 定義を変更して `default-v4l2` と `default-pipewire` を排他にする: cargo の feature システムには排他制約の仕組みがない。build.rs 側で対処する必要がある
- panic を維持してドキュメントで `--all-features` 非対応と注意書きする: 開発者体験を損なう。IDE やツールが自動的に `--all-features` を使うことがある

## 完了条件

- Linux で `cargo build --all-features` / `cargo clippy --all-features` / `cargo test --all-features` が panic せずに通る
- Linux で `cargo build --features default-v4l2,default-pipewire` も panic せずに通る（明示的な複数指定も対象）
- 既存の feature 組み合わせ（CI matrix の全パターン）の挙動が変わらない。具体的には: `""`（default → `default-v4l2`）、`--no-default-features --features default-pipewire`、`--features v4l2,pipewire`（default + v4l2,pipewire → `default-v4l2` のみ有効）
- `cargo build --workspace` が全プラットフォームで通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する（`### misc` ではなく、既存の `[FIX]` エントリと同じ階層）

## 解決方法

{完了時に記入}
