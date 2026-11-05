# crate の同梱物とパッケージ時警告を整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-package-include-targets
- Polished: {YYYY-MM-DD}

## 目的

`Cargo.toml` の `include` は `/LICENSE` / `/README.md` / `/build.rs` / `/src/**` のみで、`examples/` / `tests/` / `docs/` が公開パッケージに含まれない。

このため次の問題がある。

- `cargo package` が 6 件の警告を出す（examples 3 件、tests 3 件が「公開パッケージに含まれないため無視する」）
- README が `docs/LINUX.md` へ相対リンクしており、crates.io / docs.rs 上でリンク切れになる
- README のサンプル節が `cargo run --example ...` を案内しているが、パッケージには examples が含まれない
- `Cargo.toml` の `[[example]] name = "camera_preview"` は自動検出と同一設定で no-op であり、警告を消す効果も無い

## 優先度根拠

- Low。公開物の品質と利用者体験の問題であり、ランタイムの不具合ではない
- `/review-code` の指摘として確認済み

## 現状

`cargo package --list` の出力は 25 ファイルで、`examples/` と `docs/` を含まない。`cargo package --allow-dirty` は成功するが警告が 6 件出る。

## 設計方針

- examples / docs を同梱するなら `include` に `/examples/**` と `/docs/**` を追加する
- 同梱しないなら、README のリンクを GitHub の絶対 URL に変更し、`[[example]]` 宣言と `cargo run --example` の案内をパッケージ向けに整理する
- `[[example]]` 宣言が必要か（`required-features` 等を設定する予定が無いか）を確認し、不要なら削除する
- パッケージ時の警告を許容する場合は、その理由をコメントで残す

## 完了条件

- `cargo package` の警告が 0 件、または許容理由が明記されている
- crates.io / docs.rs 上の README でリンク切れが無い
- パッケージ内容と README の案内が一致している
- `cargo package` の検証ビルドが通る

## 解決方法

{完了時に記入}
