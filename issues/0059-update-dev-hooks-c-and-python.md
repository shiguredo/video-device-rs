# C / Objective-C 変更時にローカルフックが何も実行されず、C 用の lint・format 基盤も無い

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-dev-hooks-c-and-python
- Polished: {YYYY-MM-DD}

## 目的

`prek.toml` のローカルフック（`cargo-fmt` / `cargo-clippy` / `cargo-test`）はすべて `types = ["rust"]` である。pre-commit / pre-push フックは変更ファイルの type で発火するため、`src/video_v4l2.c` や `src/video_avf.m` だけを変更したコミット・push では、これらのフックが「no files to check」でスキップされ、ローカルでは一切コンパイルもテストも行われない。

また、リポジトリには `.clang-format` が無く、`clang-format` / `clang-tidy` / `-Wall -Wextra` の検証も無い。C / Objective-C は `cargo fmt` の対象外であり、整形・静的検査の基盤が存在しない。`canary.py` に対しても ruff / ty のフックが無く、Python コードは無検査である。

## 優先度根拠

- Low。CI が最終防衛線になっているが、ローカルの pre-push ゲートが機能していない
- `canary.py` には規約違反（末尾コメント、エラーメッセージのピリオド）が実際に残っている
- `/review-code` の指摘として確認済み

## 現状

```toml
[[repos.hooks]]
id = "cargo-test"
entry = "cargo test --workspace --all-targets"
types = ["rust"]
```

`prek run --files src/video_v4l2.c --stage pre-push --dry-run` は `cargo test ... (no files to check) Skipped` になる。pre-commit ステージでも `cargo fmt` / `cargo clippy` が同様にスキップされる。

## 設計方針

- `cargo-test` / `cargo-clippy` の `types` 制限を外すか、C / Objective-C を含む type を対象にする（C の変更はクレート全体のビルドに影響するため）
- `.clang-format` を追加し、`clang-format --dry-run --Werror` を prek と CI で実行する
- `astral-sh/ruff-pre-commit` と `astral-sh/ty-pre-commit` を追加し、`canary.py` を検査する
- フックの実行時間が問題になる場合は pre-push のみに限定する

## 完了条件

- C / Objective-C のみを変更した commit / push で `cargo test`（または少なくとも `cargo check`）が実行される
- C / Objective-C のファイルが `clang-format` で検証される
- `canary.py` が ruff / ty で検証される
- `prek run --all-files` が通る
- 開発者体験を大きく損なわない実行時間に収まる

## 解決方法

{完了時に記入}
