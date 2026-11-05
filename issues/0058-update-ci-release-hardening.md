# CI / Release の検証範囲と設定を堅牢化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-ci-release-hardening
- Polished: 2026-11-05

## 目的

CI とリリースワークフローに、検証漏れと設定の不備がある。既存の CI/CD ハードニング issue（0040: mjpeg 未検証、共有アクションの `@main` 参照、タグ形式の制限、MSRV 検証ジョブ）とは対象が異なる追加分。0040 はタグ形式を制限するが、タグ名と manifest version の一致は検証しない。本 issue はその一致検証を追加する。

1. `cargo check --workspace` / `cargo clippy --workspace` に `--all-targets` が無く、tests / examples が lint されない（`cargo test --workspace` は examples をビルドするため、コンパイルはされるが lint はされない）
2. 既定バックエンド無しの構成が CI に無い。具体的には (a) バックエンドを 1 つも有効にしない `--no-default-features` と、(b) 既定バックエンドを無効にして明示 API 用のバックエンド feature だけを有効にした `--no-default-features --features v4l2` など。現行 CI の `--no-default-features --features default-pipewire` は `default-*` を含むため (b) を検証しない
3. `cargo test -p pbt` が CI に無い。`cargo test --workspace` では feature 統合により pbt の `default-features = false` が無効化され、バックエンド無し構成がコンパイルされない
4. `cargo package` の検証が CI に無い（パッケージ同梱物の破損を検出できない）
5. CI が prek の pre-commit フック（tombi、trailing-whitespace、end-of-file-fixer、check-toml 等）を実行していない。`prek.toml` の `cargo-test` は pre-push 専用であり、CI の `cargo test` が担保する
6. `ci` / `device-test` ジョブに `permissions` が無く、既定の権限が付与される
7. `release.yml` の `github-release` / `publish` に `timeout-minutes` が無い
8. 同一ブランチへの連続 push で古い実行が並走する（`concurrency` が無い）
9. 6 つの ci matrix エントリすべてで `cargo fmt` を実行し、`cargo check` と `cargo clippy` を重複実行している
10. `ubuntu-slim` を使わず `ubuntu-24.04` 系の runner を使う理由がコメントで説明されていない。`ubuntu-slim` には Rust (rustup/cargo) が入っておらず、CI と release の cargo 実行ジョブは `ubuntu-24.04` 系の runner を必要とする
11. MSRV が `Cargo.toml` / `pbt/Cargo.toml` / `rust-toolchain.toml` / CI の `toolchain` / README の 5 箇所に重複している（ほかに `skills/shiguredo-video-device/SKILL.md` にも記載がある）。このうち `Cargo.toml` / `pbt/Cargo.toml` / `rust-toolchain.toml` は shiguredo-rust 規約が要求し、README は利用者向け情報として必要。CI の `toolchain` だけが二重管理であり、`rust-toolchain.toml` から取得する
12. リリースタグ名と `Cargo.toml` の version の一致を検証していない。`tags: ["*"]` のタグ形式制限は 0040 (3) の範囲であり、本 issue ではタグ名と manifest version の一致検証だけを追加する。`canary.py` はタグ付け前に Cargo.toml の version を canary 版へ更新するため、canary タグも一致する

## 優先度根拠

- Medium。ランタイムの不具合ではないが、品質保証とリリース事故の prevention
- (12) は誤った版の公開・表示の食い違いに直結する
- (2) の `--no-default-features --features <backend>` は 0053 の修正後に通る。0053 の完了条件にある構成を CI に追加する
- `/review-code` の指摘として確認済み

## 現状

- `.github/workflows/ci.yml` の `ci` matrix は 6 エントリあり、検証手順は `cargo fmt` / `cargo check` / `cargo test` / `cargo clippy` のみ
- `device-test` ジョブは `cargo test --workspace` を実行する
- `.github/workflows/release.yml` はタグをそのまま GitHub Release に使い、manifest との比較をしない
- `prek.toml` のフックは CI から呼ばれていない
- `cargo test -p pbt` は現行実装で成功する（実測）
- `cargo check --workspace --all-targets --no-default-features` は成功する（実測）
- `cargo check --workspace --all-targets --no-default-features --features v4l2` は 0053 の修正前は E0599 で失敗する（実測）
- `ubuntu-slim` には Rust (rustup/cargo) が入っていない（actions/runner-images の ubuntu-slim Readme）

## 設計方針

### (1) clippy の `--all-targets` と check の統合

- `ci` matrix で clippy を実行するエントリは `cargo clippy --workspace --all-targets ${{ matrix.cargo-flags }} -- -D warnings` にする
- `cargo check` ステップを削除する（clippy が check を含む）
- `cargo fmt` は prek ジョブに集約し、matrix から削除する
- `cargo test` は matrix に残す

### (2) 既定バックエンド無し構成の CI 検証

- 0053 の完了条件にある構成を `cargo check --workspace --all-targets` で検証するジョブを追加する
- 対象は `--no-default-features`、`--no-default-features --features v4l2`、`--no-default-features --features pipewire`、`--no-default-features --features v4l2,pipewire`、`--no-default-features --features mjpeg`、macOS の `--no-default-features --features avf`、Windows の `--no-default-features --features mf`
- Linux の構成では既存の `ci` ジョブと同じ `libclang-dev` / `libpipewire-0.3-dev` を、必要な feature に応じて導入する
- `--no-default-features --features <backend>` は 0053 の修正が develop に入るまで失敗するため、0053 の完了後に追加する
- 既存の `ci` matrix の `--no-default-features --features default-pipewire` は `default-*` を含むため、このジョブとは別に残す

### (3) pbt 単体と cargo package の検証

- `cargo test -p pbt` を実行するジョブを追加する
- `cargo package` を実行するジョブを追加する
- どちらも `ubuntu-24.04` で `rustup show` を実行する。`cargo package` は既定の v4l2 をビルドするため `libclang-dev` を導入する

### (4) prek フックの CI 実行

- `j178/prek-action` をコミットハッシュでピン留めし、`# vX.Y.Z` のコメントを付けて実行する
- 既定の `--all-files` で pre-commit ステージのフック（tombi、builtin、cargo fmt、cargo clippy）を実行する
- `cargo-test` は pre-push 専用のため prek ジョブでは実行せず、既存の `cargo test` で担保する
- prek ジョブは `ubuntu-24.04` で `rustup show` と `libclang-dev` の導入を行ってから実行する（cargo-clippy が既定の v4l2 をビルドするため）
- `paths-ignore: "**.md"` は既存の運用を維持する。Markdown のみの変更では CI を実行しない
- prek ジョブが Linux 既定構成の clippy を担うため、`ci` matrix の `Ubuntu (v4l2)`（x86_64）エントリから clippy を外す。fmt は (1) のとおり全 matrix エントリから削除する。`Ubuntu ARM (v4l2)`、他の feature 構成、macOS/Windows の clippy は残す

### (5) permissions / timeout / concurrency

- `ci` ワークフローに `concurrency` を設定する（`group: ${{ github.workflow }}-${{ github.ref }}`、`cancel-in-progress: true`）
- `ci` / `device-test` / 新設ジョブに `permissions: contents: read` を設定する
- `slack_notify` は `if: always() && !cancelled()` にして、concurrency によるキャンセルで失敗通知を出さない
- `release.yml` の `github-release` / `publish` / 新設の `validate` に `timeout-minutes` を設定する
- `release.yml` の `publish` に `contents: read` を追加する（checkout 用）

### (6) runner 選定理由のコメント

- `ubuntu-24.04` 系の runner を使うジョブには、`ubuntu-slim` に Rust (rustup/cargo) が入っていないため cargo 実行に `ubuntu-24.04` 系の runner が必要である旨のコメントを付ける
- `github-release` は `gh` だけを使うため `ubuntu-slim` のままでよい

### (7) MSRV の CI での取得

- `ci` ジョブに `rust-toolchain.toml` の `channel` を読み取るステップを追加し、`rust-cache` の `toolchain` 入力に渡す
- 実際のツールチェーンは `rustup show` が `rust-toolchain.toml` から導入する
- `Cargo.toml` / `pbt/Cargo.toml` / `rust-toolchain.toml` の重複は shiguredo-rust 規約が要求するため維持する
- README と `skills/shiguredo-video-device/SKILL.md` の MSRV 記載は利用者向け情報として維持する

### (8) リリースタグと manifest version の一致検証

- `release.yml` に `validate` ジョブを追加する
- `validate` は `ubuntu-24.04` で `rustup show` を実行し、`cargo metadata --no-deps --format-version 1` から `shiguredo_video_device` の version を取得する
- タグ名（`GITHUB_REF` から `refs/tags/` を除いたもの）と version を比較し、不一致なら失敗する
- `github-release` と `publish` は `validate` に依存する
- `canary.py` はタグ付け前に Cargo.toml の version を canary 版へ更新するため、canary タグも一致する。canary 用の例外は不要
- `tags: ["*"]` のタグ形式制限は 0040 (3) の範囲であり、本 issue では変更しない

## 完了条件

- CI で tests / examples が `cargo clippy --all-targets` で lint される
- 既定バックエンド無し構成（bare、v4l2、pipewire、v4l2,pipewire、mjpeg、avf、mf）が CI で `cargo check --all-targets` される。bare は現行でも通る。explicit-backend は 0053 の修正が前提
- `cargo test -p pbt` と `cargo package` が CI で実行される
- CI が prek の pre-commit フック集合を実行する（cargo-test は既存の cargo test で担保）
- CI ワークフローに concurrency、全ジョブに permissions と timeout-minutes が設定されている
- `release.yml` の `validate` がタグと manifest version の不一致で失敗し、`github-release` / `publish` が `validate` に依存する
- `ubuntu-24.04` 系の runner を使う理由（ubuntu-slim に Rust が無い）がコメントで残る
- CI の `rust-cache` の `toolchain` が `rust-toolchain.toml` の `channel` から取得される
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する
- 既存の CI matrix が引き続き通る（0053 の修正後に no-default explicit-backend を追加する）

## 解決方法

{完了時に記入}
