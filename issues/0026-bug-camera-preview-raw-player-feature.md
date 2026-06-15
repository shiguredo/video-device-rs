# README が指示する `--features raw-player` が Cargo.toml に存在しない

- Priority: Medium
- Created: 2026-06-15
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/fix-readme-raw-player-feature
- Polished: 2026-06-15

## 目的

`README.md` の `camera_preview` 例の案内が `cargo run --features raw-player --example camera_preview` を提示しているが、`Cargo.toml` の `[features]` セクションに `raw-player` という feature は存在しない。利用者が README の手順をそのままコピペすると `error: Package does not have these features: raw-player` で失敗する。`raw_player` クレートが `[dev-dependencies]` に直書きされており feature gate を経由しないため、`--features raw-player` 自体が意味を持たない。README、`examples/README.md`、および `examples/camera_preview.rs` の doc コメントから `--features raw-player` を削除して、実際に動くコマンドに揃える。

## 優先度根拠

- Medium。直接のメモリ安全性・機能不全ではないが、README の最初の入り口で利用者を躓かせるドキュメント不整合
- `Cargo` の挙動として、存在しない feature 名を `--features` で指定すると `error: Package does not have these features: raw-player` を返す。コピペで即エラーになる
- `/review-code` の致命的指摘として確認された README と Cargo.toml の不整合
- `examples/README.md` と `README.md` で同じ `camera_preview` のコマンドが食い違っている (前者は feature 無指定、後者は `--features raw-player` 付き)

## 現状

`Cargo.toml` の `[features]` (関連抜粋):

```toml
[features]
default = ["default-v4l2", "default-avf", "default-mf"]
avf = []
mf = ["dep:windows"]
pipewire = []
v4l2 = []
default-avf = ["avf"]
default-mf = ["mf"]
default-pipewire = ["pipewire"]
mjpeg = ["v4l2"]
default-v4l2 = ["v4l2"]
```

`raw-player` という feature は存在しない。

`[dev-dependencies]`:

```toml
[dev-dependencies]
# 依存ゼロの軽量 JSON パーサ／シリアライザ。サンプル用途で利用
nojson = "0.3"
# camera_preview サンプルで生フレームを表示するためのプレビュー用クレート
raw_player = "=2026.1.0"
```

`raw_player` は `dev-dependencies` として直書きされており、`cargo run --example camera_preview` を実行した時点で常に解決される。feature gate を経由しないため、`--features raw-player` の指定があってもなくても `raw_player` クレートはビルドされる。

`README.md` の該当箇所 (`## 使い方` セクション以下の `camera_preview` 案内):

```markdown
カメラ映像をキャプチャして raw-player でプレビュー表示する。`raw-player` feature が必要。

```bash
cargo run --features raw-player --example camera_preview
cargo run --features raw-player --example camera_preview -- --resolution 1080p --fps 60
```
```

`examples/README.md`:

```bash
cargo run --example camera_preview -- --list-devices
cargo run --example camera_preview
cargo run --example camera_preview -- --resolution 1080p --fps 60
```

`examples/camera_preview.rs` 冒頭の doc コメントは `cargo run --example camera_preview ...` のみを案内しており、`raw-player` には触れていない。

つまり README.md だけが存在しない feature を案内している不整合状態にある。

## 設計方針

README / `examples/README.md` / `examples/camera_preview.rs` の案内コマンドを **`--features raw-player` を含まない形** に統一する。

具体的には:

- `README.md` の該当節から `--features raw-player` を削除し、説明文も「`raw-player` feature が必要」を削除する。プレビュー機能の実装が `dev-dependencies` の `raw_player` クレートを利用していること自体は文章として残してよい
- `examples/README.md` は既に `--features raw-player` を含まない形なので変更不要
- `examples/camera_preview.rs` の doc コメントは既に `--features raw-player` を含まないので変更不要

### 採用しない案

- **`raw-player` feature を `Cargo.toml` に実体化する案**: `[features]` に `raw-player = []` を追加し、`[[example]] name = "camera_preview"` に `required-features = ["raw-player"]` を加える方法。`raw_player` クレートは `[dev-dependencies]` に直書きされていて feature 経由で解決されないため、feature を追加しても **依存解決には何の効果もない**。`required-features` の唯一の効果は「`cargo run --example camera_preview` (feature 無指定) で `error: the example target 'camera_preview' requires the features: raw-player` のエラーになる」ことだが、これは利用者にとってコマンドが増えるだけのデメリットしかない。「将来 SDL 系の重い依存を追加する際に feature gate を維持できる」という根拠は推測 (Premature Optimization is the Root of All Evil) であり、現状は採用しない
- **`raw_player` を `optional dev-dependencies` にする案**: Cargo の制約上、`dev-dependencies` に `optional = true` は付けられないため採用不可

## 影響範囲

- `README.md`: `camera_preview` 例の案内コマンドから `--features raw-player` を削除し、説明文を更新する
- `examples/README.md`: 変更なし (既に整合)
- `examples/camera_preview.rs`: 変更なし (既に整合)
- `Cargo.toml`: 変更なし (新たな feature は追加しない)。`[[example]] name = "camera_preview"` の空セクションは現状 `required-features` も `path` 指定もなく実質的に Cargo のデフォルト探索と等価だが、本 issue のスコープ外として削除しない (削除は別 issue で扱う)
- 公開 API: 変更なし
- 他の `open` issue (0027 / 0028) との依存はなし

## 完了条件

- `README.md` の `camera_preview` 例の案内から `--features raw-player` を削除すること
- 説明文「`raw-player` feature が必要」を削除すること
- `cargo run --example camera_preview` (feature 無指定) が正常にビルド・起動すること (環境にカメラがある場合は実行も完了すること、ない場合はカメラ未検出のエラーで終了することは許容)
- `cargo run --example camera_preview -- --list-devices` がデバイス一覧表示で正常終了すること
- `cargo run --example camera_preview -- --resolution 1080p --fps 60` のオプション付き起動が引数解析エラーを起こさないこと
- `cargo build --all-targets` および `cargo build --workspace --all-targets` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- 既存の `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] README で案内している camera_preview 例の --features raw-player が Cargo.toml に存在しない不整合を解消する`)。担当者行 (`- @<github-id>`) を含める

## 解決方法

{完了時に記入}
