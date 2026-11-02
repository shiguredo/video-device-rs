# 死にコード・未使用コード・到達不能コードを削除する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-dead-code-cleanup
- Polished: {YYYY-MM-DD}

## 目的

コードベース全体から、不要なコードを削除する。挙動は変えない。

1. `src/video_pipewire.c` の `negotiated_stride` フィールドは宣言のみで読み書きが無い
2. `src/video_pipewire.c` の `PendingNode::device_index` は代入のみで読み出しが無い（`ctx->count` と配列順で自明）
3. `src/video_pipewire.c` の `PendingNode::param_done` は 3 箇所で代入されるが読み出しが無い
4. `src/video_pipewire.c` の `#include <stdio.h>` は stdio 関数を使用していない
5. `src/video_pipewire.c` の `#include <spa/debug/types.h>` は `spa_debug_*` を使用していない
6. `src/video_pipewire.c` の `spa_zero(p->node_listener)` は直前の `memset(p, 0, sizeof(*p))` と重複している
7. `build.rs` の `.derive_default(true)` / `.derive_debug(true)` は生成型の `Default` / `Debug` を使用していない
8. `src/capture_mf.rs` の `PixelFormat::Unknown` チェックは到達不能（`get_configured_format` が未知 GUID を既にエラーにする）
9. `src/video_v4l2.c` / `src/video_pipewire.c` の `if (session->callback)` は、`session_start` が callback 非 NULL を要求した後にのみスレッドが動くため到達不能
10. `.markdownlint.jsonc` は prek / CI / Makefile のどこからも参照されていない。フックを追加するか設定を削除する
11. `src/frame_math.rs` の `mjpeg_payload_bytes` は符号チェック付きキャストのみで、1 箇所からしか呼ばれていない。PBT で代替できるなら削除候補
12. `src/types.rs` の `PixelBuffer::as_ptr` はリポジトリ内で未使用。公開 API として残すなら用途を doc に書く
13. `src/capture_ffi.rs` / `src/device_ffi.rs` の private な `CaptureOps` / `DeviceOps` のフィールドに不要な `pub` が付いている。`pub(crate)` 型のメソッドに `pub` を付けている箇所もあり、可視性の意図が読めない
14. `FfiCaptureImpl` / `MfCaptureImpl` の `impl` ブロックが 2 つに分割されており、`new` と `start` / `stop` の対応を追いにくい
15. `src/types.rs` の `PixelBuffer::from_retained_ptr` の `debug_assert!(ptr.is_null(), ...)` は直前の NULL チェックの後なので条件が常に偽になる。意図が「契約違反の検出」なら `debug_assert!(false, ...)` 等に整理する
16. `build.rs` の `target_os` の `match` が 2 回あり、2 回目の `_` アームは到達不能。match の網羅性のため削除はできないので `unreachable!` 化するか分岐を統合する

## 優先度根拠

- Low。いずれも挙動に影響しない整理
- 死にコードは「Don't live with broken windows」に反し、将来の修正時に誤解を招く
- `/review-code` の削除候補として 3 周にわたり確認済み

## 現状

`rg` で確認済み。`negotiated_stride` / `device_index` / `param_done` は宣言以外に読み出しが無い。`stdio.h` / `spa/debug/types.h` の使用シンボルも無い。`build.rs` の 2 回目の `match target_os` の `_` アームは到達不能だが、match の網羅性のため削除はできない（`unreachable!` 化または構成分割で整理する）。

## 設計方針

- 各項目を削除または統合する
- `.markdownlint.jsonc` は markdownlint フックを追加するか、設定ファイルごと削除する
- 到達不能な `_` アームは `unreachable!` に置き換えるか、`target_os` の分岐を 1 回に統合する
- `PixelBuffer::as_ptr` を残す場合は用途と有効期間を doc に明記する

## 完了条件

- 上記 1〜16 が削除または整理されている
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- 挙動が変わっていない

## 解決方法

{完了時に記入}
