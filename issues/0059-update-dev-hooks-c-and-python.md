# C / Objective-C / Python 変更時にローカルフックが実行されず、C / Python の lint・format 基盤も無い

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-dev-hooks-c-and-python
- Polished: 2026-11-05

## 目的

`prek.toml` のローカルフック（`cargo-fmt` / `cargo-clippy` / `cargo-test`）はすべて `types = ["rust"]` である。pre-commit / pre-push フックは変更ファイルの type で発火するため、`src/video_v4l2.c` や `src/video_avf.m` だけを変更したコミット・push では、これらのフックが「no files to check」でスキップされ、ローカルでは一切コンパイルもテストも行われない。

また、リポジトリには `.clang-format` が無く、`clang-format` による整形チェックも無い。`clang-tidy` も実行していない。C / Objective-C は `cargo fmt` の対象外であり、整形基盤が存在しない。`-Wall -Wextra` は `cc` クレートの既定で付与されるが、`warnings_into_errors` を設定していないため、警告はエラーにならず、警告の混入を防げない。`canary.py` に対しても ruff / ty のフックが無く、Python コードは無検査である。

## 優先度根拠

- Low。CI の `cargo test` / `cargo clippy` は C / Objective-C をビルドするため最終防衛線にはなっているが、ローカルの pre-push ゲートが機能していない
- `canary.py` には規約違反（末尾コメント、エラーメッセージのピリオド、`Optional[str]`）が残っており、`issues/0061-update-canary-py-conventions.md` で修正する。本 issue は ruff / ty のフックを追加し、ruff で検出できる規約違反の再発を防ぐ
- `/review-code` の指摘として確認済み

## 現状

- `prek.toml` の `cargo-fmt` / `cargo-clippy` / `cargo-test` は `types = ["rust"]` である
- `prek run --files src/video_v4l2.c --stage pre-push --dry-run` は `cargo test ... (no files to check) Skipped` になる（実測）
- `prek run --files src/video_v4l2.c --stage pre-commit --dry-run` でも `cargo fmt` / `cargo clippy` がスキップされる（実測）
- `.clang-format` は存在しない。`clang-format --dry-run --Werror` を全 C / Objective-C ファイルに実行すると失敗する（実測）。既存コードは未整形である
- `build.rs` の `cc::Build` は `-Wall -Wextra` を付与するが、`warnings_into_errors` を設定していない。`cargo clippy -- -D warnings` は Rust の警告だけをエラーにする
- `clang-tidy` は実行していない
- `canary.py` に対して ruff / ty のフックは無い
- `uvx ruff check --isolated canary.py` は UP045 (`Optional[str]`) を検出する（実測）。`uvx ruff format --isolated --check canary.py` と `uvx --from ty ty check canary.py` は通る（実測）
- `.github/workflows/ci.yml` は prek を実行していない。prek の CI 実行は `issues/0058-update-ci-release-hardening.md` で追加する

## 設計方針

### 1. C / Objective-C 変更時に cargo-clippy / cargo-test を実行する

- `cargo-clippy` と `cargo-test` の `types` を `types_or = ["rust", "c", "c++", "objective-c"]` に変更する。`types_or` は prek で利用できる（`prek validate-config` で確認済み）。`.h` は `c` / `c++`、`.m` は `objective-c` として判定される
- `cargo-fmt` は Rust のみを整形するため `types = ["rust"]` のままにする
- `cargo-test` は pre-push のみ、`cargo-clippy` は pre-commit のままにする。C 変更時の clippy はクレート全体のビルドを伴うが、Rust 変更時と同じ範囲であり、stage は変更しない
- `cargo-clippy` は priority 10、`cargo-test` は priority 20 のままにする

### 2. C / Objective-C の整形チェック

- `.clang-format` を追加する。既存コードのスタイル（4 スペースインデント、K&R ブレース、ポインタは型側、行長制限なし、日本語コメントの再折り返しなし）に合わせ、C / C++ と Objective-C の 2 セクションに分ける。基準は `BasedOnStyle: LLVM`、`IndentWidth: 4`、`IndentCaseLabels: true`、`ColumnLimit: 0`、`PointerAlignment: Left`、`BreakBeforeBraces: Attach`、`ReflowComments: false`、`ObjCSpaceAfterProperty: true` とし、整形後の差分が最小になるように調整する
- `pre-commit/mirrors-clang-format` を `rev` で固定して prek に追加する。`args = ["--dry-run", "--Werror", "-style=file"]` にして check-only で実行する。フックの `entry` は `clang-format -i` だが、`--dry-run` を渡すためファイルは変更されない（実測）。`clang-format` のバージョンはフックの `rev` で固定されるため、ローカルと CI で同じバージョンを使う。実装時点の最新タグは `v23.1.5`
- `clang-format` は priority 0 とする
- `src/video_v4l2.c`、`src/video_pipewire.c`、`src/video_avf.m`、`src/video.h`、`src/video_v4l2.h`、`src/video_pipewire.h`、`src/video_avf.h` を `.clang-format` で整形する。`clang-format --dry-run --Werror` が全ファイルで通ることを確認する

### 3. C / Objective-C の警告をエラーにする

- `build.rs` の `build_macos` / `build_linux_v4l2` / `build_linux_pipewire` にある各 `cc::Build` に `warnings_into_errors(true)` を追加し、`-Wall -Wextra` の警告をエラーにする。`src/video_v4l2.c` は `-Wall -Wextra -Werror` で警告 0 件であることを確認済み
- `src/video_pipewire.c` と `src/video_avf.m` は CI の Linux (pipewire) と macOS で警告 0 件を確認し、必要なら警告を修正する
- `warnings_into_errors(true)` はビルドスクリプトで常時有効にする。利用者環境でも C / Objective-C の警告があればビルドが失敗する点は、品質を優先する方針として許容する
- `clang-tidy` は `compile_commands.json` の生成が必要なため、本 issue では対象外とする。コンパイラ警告をエラーにすることで C / Objective-C の lint の最低限を担保する

### 4. Python の lint / format / 型検査

- `astral-sh/ruff-pre-commit` と `astral-sh/ty-pre-commit` を `rev` で固定して prek に追加する。`priority` で `ruff-format` → `ruff-check` → `ty` の順にする。実装時点の最新タグは ruff が `v0.17.3`、ty が `v0.0.97`
- `ruff-format` は priority 0、`ruff-check` は priority 10、`ty` は priority 20 とする
- `ruff-format` は Markdown 内の Python コードブロックも対象になる（ruff-pre-commit の `types_or` に `markdown` が含まれる）。`prek run --all-files` では Markdown も整形対象になることに注意する。現状の Markdown は整形済みである（`uvx ruff format --isolated --check .` で確認済み）
- `ty` は `always_run: true` で Python ファイルの変更が無くても実行される。`uv check` を呼ぶため、開発者と CI に uv が必要である
- `canary.py` は `ruff-check` の UP045 (`Optional[str]`) を解消する必要がある。`issues/0061-update-canary-py-conventions.md` の修正が完了していればそのまま通る。未完了なら本 issue で `Optional[str]` を `str | None` に修正する（0061 と重複するが、0061 側で残りの規約違反を扱う）
- `pytest` フックは追加しない。canary.py 用の Python テストが無く、pytest はテスト未収集で失敗するため
- `pyproject.toml` は追加しない。Python パッケージではなく、ruff / ty は既定設定で canary.py を検査できる（`uvx ruff check --isolated canary.py`、`uvx --from ty ty check canary.py` で確認済み）。`uv check` は pyproject.toml 不在の警告を出すが、型検査は動作する

### 5. CI での実行

- CI では `issues/0058-update-ci-release-hardening.md` で追加する `j178/prek-action` の prek ジョブが `prek.toml` のフックを実行する。本 issue では `.github/workflows/ci.yml` に clang-format / ruff / ty の個別ステップを追加しない
- `ty` フックが `uv check` を呼ぶため、0058 の prek ジョブに `astral-sh/setup-uv` を追加する必要がある。0058 の実装時に併せて追加する
- 0058 が未完了の間は、本 issue の CI 実行は有効にならない。ローカルでは `prek run --all-files` で確認する

### 6. 設定の検証

- 設定変更後に `prek validate-config` と `prek install --prepare-hooks` を実行する

## 完了条件

- C / Objective-C のみを変更した pre-commit で `cargo clippy` が Skip されず、pre-push で `cargo test` が Skip されない。`prek run --files src/video_v4l2.c --stage pre-commit --dry-run` と `prek run --files src/video_v4l2.c --stage pre-push --dry-run` で確認する
- `prek run --files src/video_avf.m --stage pre-push --dry-run` でも `cargo test` が Skip されない
- `.clang-format` が追加され、全 C / Objective-C ファイルが `clang-format --dry-run --Werror` を通る
- `prek run --all-files` が通る（clang-format、ruff-format、ruff-check、ty、cargo-fmt、cargo-clippy、tombi、builtin を含む）
- `prek run --all-files --stage pre-push` が通る（cargo-test を含む）
- `prek validate-config` が通る
- `build.rs` の `cc::Build::warnings_into_errors(true)` により、C / Objective-C の `-Wall -Wextra` 警告がエラーになる。Linux (v4l2 / pipewire) と macOS の CI で警告 0 件を確認する
- `canary.py` が `ruff check` / `ruff format --check` / `ty check` を通る
- `prek run --all-files` の実行時間が開発者体験を大きく損なわない範囲に収まる。cargo-test は pre-push のみ、cargo-clippy は pre-commit のままとする
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する

## 解決方法

{完了時に記入}
