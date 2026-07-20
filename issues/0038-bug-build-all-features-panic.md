# cargo --all-features が Linux で panic する

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-build-all-features-panic
- Polished: {YYYY-MM-DD}

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

`--all-features` は全 feature を有効化するため、Linux で `default-v4l2` と `default-pipewire` の両方が有効になり panic する。macOS では `default-pipewire` / `default-v4l2` が build.rs の `match target_os` で無視されるため発生しない。Windows も同様。

## 設計方針

`--all-features` 時の挙動を「panic ではなく、最初の default を優先」に変更する。

1. `enable_default_count >= 2` の場合、panic する代わりに stderr に警告を出し、優先順位（`default-v4l2` > `default-pipewire`）で最初の 1 つだけを有効にする
2. または、`--all-features` 検出時に default 系 feature を全て無視し、非 default の feature（`v4l2`, `pipewire`）だけを有効にする

### 採用しない案

- Cargo.toml の feature 定義を変更して `default-v4l2` と `default-pipewire` を排他にする: cargo の feature システムには排他制約の仕組みがない。build.rs 側で対処する必要がある
- panic を維持してドキュメントで `--all-features` 非対応と注意書きする: 開発者体験を損なう。IDE やツールが自動的に `--all-features` を使うことがある

## 完了条件

- Linux で `cargo build --all-features` / `cargo clippy --all-features` / `cargo test --all-features` が panic せずに通る
- 既存の feature 組み合わせ（CI matrix の全パターン）の挙動が変わらない
- `cargo build --workspace` が全プラットフォームで通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` の `### misc` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
