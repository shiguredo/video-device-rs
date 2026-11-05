# CI / Release の検証範囲と設定を堅牢化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-ci-release-hardening
- Polished: {YYYY-MM-DD}

## 目的

CI とリリースワークフローに、検証漏れと設定の不備がある。既存の CI/CD ハードニング issue（mjpeg 未検証、共有アクションの `@main` 参照、任意タグでのリリース、MSRV 検証ジョブ）とは対象が異なる追加分。

1. `cargo check --workspace` / `cargo clippy --workspace` に `--all-targets` が無く、tests / examples が lint されない（`cargo test --workspace` は examples をビルドする）
2. `--no-default-features` 構成（既定バックエンド無し、明示 API のみ）が CI に無い
3. `cargo test -p pbt` が CI に無い。`cargo test --workspace` では feature 統合により pbt の `default-features = false` が無効化され、バックエンド無し構成がコンパイルされない
4. `cargo package` の検証が CI に無い（パッケージ同梱物の破損を検出できない）
5. CI が prek のフック（tombi、trailing-whitespace、end-of-file-fixer、check-toml 等）を実行していない
6. `ci` / `device-test` ジョブに `permissions` が無く、既定の権限が付与される
7. `release.yml` の `github-release` / `publish` に `timeout-minutes` が無い
8. 同一ブランチへの連続 push で古い実行が並走する（`concurrency` が無い）
9. 6 ジョブすべてで `cargo fmt` を実行し、`cargo check` と `cargo clippy` を重複実行している
10. `ubuntu-slim` を使わず `ubuntu-24.04` を使う理由がコメントで説明されていない
11. MSRV が `Cargo.toml` / `pbt/Cargo.toml` / `rust-toolchain.toml` / CI の `toolchain` / README の 5 箇所に重複している
12. リリースタグ名と `Cargo.toml` の version の一致を検証していない。`tags: ["*"]` で任意タグを受理し、GitHub Release 名はタグ、crates.io は manifest の version になるため、両者がずれると公開物と表示が食い違う

## 優先度根拠

- Medium。ランタイムの不具合ではないが、品質保証とリリース事故の prevention
- (12) は誤った版の公開・表示の食い違いに直結する
- `/review-code` の指摘として確認済み

## 現状

- `.github/workflows/ci.yml` の検証手順は `cargo fmt` / `cargo check` / `cargo test` / `cargo clippy` のみ
- `.github/workflows/release.yml` はタグをそのまま GitHub Release に使い、manifest との比較をしない
- `prek.toml` のフックは CI から呼ばれていない

## 設計方針

- CI の clippy / check に `--all-targets` を付ける
- `--no-default-features` 構成、`cargo test -p pbt`、`cargo package` の検証ジョブを追加する
- CI で `j178/prek-action` を実行し、ローカルと同じフック集合にする
- ジョブに `permissions: contents: read`、`timeout-minutes`、`concurrency` を設定する
- `cargo fmt` は専用ジョブに集約し、`cargo check` は clippy に統合する
- runner 選定理由をコメントで残す
- MSRV は CI が `rust-toolchain.toml` から取得する形に寄せる
- リリース時に `cargo metadata` の version とタグ名を比較し、不一致なら失敗させる

## 完了条件

- CI で tests / examples が clippy される
- 既定バックエンド無し構成、pbt 単体、cargo package が CI で検証される
- CI とローカルの prek フック集合が一致する
- 各ジョブに permissions / timeout / concurrency が設定されている
- タグと manifest version が不一致のリリースが失敗する
- 既存の CI matrix が引き続き通る

## 解決方法

{完了時に記入}
