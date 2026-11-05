# CHANGES.md の 2026.4.0 セクションを規約に合わせる

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-changelog-develop-cleanup
- Polished: 2026-11-06

## 目的

`CHANGES.md` の `## 2026.4.0` セクションに、shiguredo-changelog 規約（派生元ブランチとの最終差分のみ記載する、中間状態の修正は記載しない、種別順を守る）に反する記述が残っている。issue 作成時は `## develop` だったが、2026-11-05 のリリースで `## 2026.4.0` になった。

1. `### misc` の ``- [UPDATE] `#[ignore]` に実機デバイスが必要な理由を明示する`` が、同じ `### misc` の
   ``- [UPDATE] 実機デバイスが必要なテストを `#[ignore]` ではなく環境変数 `VIDEO_DEVICE_DEVICE_TEST` で判定するようにする``
   と矛盾している。現コードに `#[ignore]` 属性は無く、`tests/helpers/device_test.rs` の `device_test_enabled()` が
   `VIDEO_DEVICE_DEVICE_TEST` で実行可否を判定している。中間状態の記述なので削除する
2. `### misc` の ``- [UPDATE] `Error` に `Copy` を実装し、全バリアントにドキュメントを追加する`` は公開 API の追加であり、
   ``- [UPDATE] `PixelFormat` のメソッドを `self` 受けに統一する`` は公開 API の変更である。
   `### misc`（機能に直接影響しない変更）ではなく本体セクション（`[ADD]` / `[UPDATE]`）が妥当
3. 冒頭の種別一覧が `UPDATE` → `ADD` → `CHANGE` → `FIX` の順で、エントリの推奨順（`CHANGE` → `ADD` → `UPDATE` → `FIX`）と異なる

## 優先度根拠

- Low。リリースノートの品質問題であり、ランタイムには影響しない
- 矛盾した記述が残ると、利用者が変更内容を誤解する
- `/review-code` の重要指摘（規約違反）として確認済み

## 現状

- `## develop` は空で、問題のある記述は 2026-11-05 にリリースされた `## 2026.4.0` にある
- `## 2026.4.0` の `### misc` には次の 2 つのエントリが併記されている
  - ``- [UPDATE] 実機デバイスが必要なテストを `#[ignore]` ではなく環境変数 `VIDEO_DEVICE_DEVICE_TEST` で判定するようにする``
  - ``- [UPDATE] `#[ignore]` に実機デバイスが必要な理由を明示する``
- 現コードに `#[ignore]` 属性は無い。`tests/helpers/device_test.rs` の `device_test_enabled()` が `VIDEO_DEVICE_DEVICE_TEST` の有無で実行可否を判定し、`tests/test_capture.rs` / `tests/test_pipewire.rs` がこの関数を使っている
- `## 2026.4.0` の `### misc` には ``- [UPDATE] `Error` に `Copy` を実装し、全バリアントにドキュメントを追加する`` と ``- [UPDATE] `PixelFormat` のメソッドを `self` 受けに統一する`` がある
- `Error` は `src/error.rs` の `#[derive(Debug, Clone, Copy)] pub enum Error` で定義され、`src/lib.rs` の `pub use error::{Error, Result}` で公開されている。`Copy` の実装は公開 API の追加である
- `PixelFormat` は `src/types.rs` の `pub enum PixelFormat` で定義され、
  `src/lib.rs` の `pub use types::{ PixelBuffer, PixelFormat, ... }` で公開されている。
  `pub fn to_raw(self)` / `pub fn name(self)` は `self` 受けであり、公開 API の変更である
- 冒頭の種別一覧は `UPDATE` → `ADD` → `CHANGE` → `FIX` の順で、shiguredo-changelog の推奨順 `CHANGE` → `ADD` → `UPDATE` → `FIX` と異なる
- issue 作成時に指摘した次の項目は、リリースまでに解消済み
  - `Cargo.toml` の `windows` 依存を MSRV 1.93 の cargo が解釈できる形式に修正する `[FIX]` エントリは、リリースコミットで削除済み。現在の `CHANGES.md` にこのエントリは無い
  - `### misc` 内のエントリ間の空行はリリースコミットで削除済み。現在の `### misc` は他セクションと同じく空行を挟まない
  - `raw_player` は最終的に `2026.3` となり、``- [UPDATE] camera_preview サンプルを raw_player 2026.3 に追従する`` として記載済み。`=2026.2.0` → `2026.2` は中間状態であり、最終差分には含まれない

## 設計方針

- `## 2026.4.0` の `### misc` から ``- [UPDATE] `#[ignore]` に実機デバイスが必要な理由を明示する`` を削除する。最終状態では `#[ignore]` を使わず、`VIDEO_DEVICE_DEVICE_TEST` で判定しているため
- ``- [UPDATE] `Error` に `Copy` を実装し、全バリアントにドキュメントを追加する`` を本体セクションの `[ADD]` に移す。公開 API の追加であり、`### misc` の対象ではない。全バリアントのドキュメント追加は `Copy` 実装に付随するため、同じエントリに含めたままでよい
- ``- [UPDATE] `PixelFormat` のメソッドを `self` 受けに統一する`` を本体セクションの `[UPDATE]` に移す。公開 API の変更であり、`### misc` の対象ではない
- 本体セクションのエントリ順を `CHANGE` → `ADD` → `UPDATE` → `FIX` にする。`Error` の `Copy` は `[ADD]`、`PixelFormat` の `self` 受けは `[UPDATE]` として、MSRV の `[CHANGE]` の後に置く
- 冒頭の種別一覧を `CHANGE` → `ADD` → `UPDATE` → `FIX` の順に並べ替える
- `### misc` には機能に直接影響しない変更（PBT、ドキュメント、リファクタリング、Makefile、examples）だけを残す
- `Cargo.toml` の `windows` 依存の記述形式の変更は 2026.3.0 からの最終差分に含まれるが、利用者向けの機能変更ではないため、`### misc` には追加しない。リリース時に削除された判断を維持する
- 2026.4.0 セクションはリリース済みだが、`CHANGES.md` は develop 上で修正する。リリースタグ 2026.4.0 の内容は変更しない

## 完了条件

- `## 2026.4.0` の `### misc` から `#[ignore]` のエントリが削除されている
- `Error` の `Copy` 実装のエントリが本体セクションの `[ADD]` に移り、`PixelFormat` の `self` 受け統一のエントリが本体セクションの `[UPDATE]` に移っている
- 冒頭の種別一覧とエントリの種別順が `CHANGE` → `ADD` → `UPDATE` → `FIX` になっている
- `## 2026.4.0` の内容が 2026.3.0 からの最終差分だけを含み、中間状態の修正を含まない
- `.markdownlint.jsonc` が存在する場合は、それを指定して markdownlint を実行し、`CHANGES.md` に問題が出ない

## 解決方法

{完了時に記入}
