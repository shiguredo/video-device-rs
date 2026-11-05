# 死にコード・未使用コード・到達不能コードを整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-dead-code-cleanup
- Polished: 2026-11-06

## 目的

コードベース全体から、不要なコード・設定・可視性・構造を整理する。挙動は変えない。

1. `src/video_pipewire.c` の `negotiated_stride` フィールドは宣言のみで読み書きが無い
2. `src/video_pipewire.c` の `PendingNode::device_index` は代入のみで読み出しが無い（`ctx->count` と配列順で自明）
3. `src/video_pipewire.c` の `PendingNode::param_done` は 3 箇所で代入されるが読み出しが無い
4. `src/video_pipewire.c` の `#include <stdio.h>` は stdio 関数を使用していない
5. `src/video_pipewire.c` の `#include <spa/debug/types.h>` は `spa_debug_*` を使用していない
6. `src/video_pipewire.c` の `spa_zero(p->node_listener)` は直前の `memset(p, 0, sizeof(*p))` と重複している
7. `build.rs` の `.derive_default(true)` / `.derive_debug(true)` は生成型の `Default` / `Debug` を使用していない
8. `src/capture_mf.rs` の `MfCaptureImpl::new` にある `matches!(pixel_format, PixelFormat::Unknown(_))` チェックは到達不能（`get_configured_format` が未知 GUID を `Error::SessionCreateFailed` として既にエラーにする）。`process_sample` の `PixelFormat::Unknown(_) => return` アームも到達不能だが、`match` の網羅性のため残す
9. `src/video_v4l2.c` / `src/video_pipewire.c` の `if (session->callback)` は、`session_start` が callback 非 NULL を要求した後にのみスレッドが動くため到達不能
10. `.markdownlint.jsonc` は prek / CI / Makefile のどこからも参照されていない。設定ファイルごと削除する
11. `src/frame_math.rs` の `mjpeg_payload_bytes` は符号チェック付きキャストのみで、`src/capture_ffi.rs` の `frame_callback` から 1 箇所でしか呼ばれていない。ただし、`stride` の符号チェックを純粋関数として分離し、単体テストで境界を検証している。PBT は公開 API 経由でしかテストできないため、private 関数の単体テストを代替できない。よって削除しない
12. `src/types.rs` の `PixelBuffer::as_ptr` は Rust コード内では未使用だが、`skills/shiguredo-video-device/SKILL.md` に使用例がある公開 API。`src/types.rs` の doc に用途と有効期間を明記する
13. `src/capture_ffi.rs` / `src/device_ffi.rs` の private な `CaptureOps` / `DeviceOps` のフィールドに不要な `pub` が付いている。`CaptureContext` の `callback` フィールドも同様。`FfiCaptureImpl` / `FfiDeviceImpl` / `FfiDeviceListImpl` は `pub(crate)` 型なのにメソッドに `pub` が付いており、可視性の意図が読めない。フィールドの `pub` を外し、メソッドを `pub(crate)` に統一する
14. `FfiCaptureImpl` / `MfCaptureImpl` の `impl` ブロックが 2 つに分割されており、`new` と `start` / `stop` の対応を追いにくい。1 つの `impl` ブロックに統合する
15. `src/types.rs` の `PixelBuffer::from_retained_ptr` の `debug_assert!(ptr.is_null(), ...)` は直前の NULL チェックの後なので条件が常に偽になる。到達した時点で非 NULL が確定しているため、`debug_assert!(false, ...)` に置き換える（デバッグビルドでの契約違反検出は維持する）
16. `build.rs` の `target_os` の `match` が 2 回あり、2 回目の `_` アームは到達不能（1 回目の `match` が未対応 OS で panic するため）。`unreachable!()` に置き換える。`target_os` の分岐統合は行わない

## 優先度根拠

- Low。いずれも挙動に影響しない整理
- 死にコードは「Don't live with broken windows」に反し、将来の修正時に誤解を招く
- `/review-code` の削除候補として 3 周にわたり確認済み

## 現状

`rg` で確認済み。`negotiated_stride` / `device_index` / `param_done` は宣言以外に読み出しが無い。`stdio.h` / `spa/debug/types.h` の使用シンボルも無い。`spa_zero(p->node_listener)` は直前の `memset(p, 0, sizeof(*p))` と重複している。`build.rs` の `.derive_default(true)` / `.derive_debug(true)` は生成型の `Default` / `Debug` を使用していない。

`src/capture_mf.rs` の `MfCaptureImpl::new` の `PixelFormat::Unknown` チェックは `get_configured_format` が未知 GUID を `Error::SessionCreateFailed` としてエラーにするため到達不能で、`process_sample` の `PixelFormat::Unknown(_) => return` アームも到達不能（ただし `match` の網羅性のため残す）。`src/video_v4l2.c` / `src/video_pipewire.c` の `if (session->callback)` は `session_start` が callback 非 NULL を要求した後にのみスレッドが動くため到達不能。`.markdownlint.jsonc` は prek / CI / Makefile のどこからも参照されていない。

`mjpeg_payload_bytes` は `src/capture_ffi.rs` の `frame_callback` から 1 箇所でしか呼ばれていないが、`src/frame_math.rs` の単体テストで境界を検証している。PBT は `pbt/tests/` の `prop_error.rs` / `prop_types.rs` だけで、`frame_math` の private 関数を直接テストできない。`PixelBuffer::as_ptr` は Rust コード内では未使用だが、`skills/shiguredo-video-device/SKILL.md` に使用例がある。

`CaptureOps` / `DeviceOps` のフィールドと `CaptureContext::callback` に不要な `pub` があり、`FfiCaptureImpl` / `FfiDeviceImpl` / `FfiDeviceListImpl` のメソッドにも `pub` が付いている。`FfiCaptureImpl` / `MfCaptureImpl` の `impl` ブロックは 2 つに分割されている。`from_retained_ptr` の `debug_assert!(ptr.is_null(), ...)` は NULL チェック後なので条件が常に偽になる。`build.rs` の 2 回目の `match target_os` の `_` アームは到達不能だが、`match` の網羅性のため削除はできない。

## 設計方針

- 各項目を削除・統合・現状維持、または下記の具体的な修正のいずれかにする。現状維持の場合は理由を明記する
- `.markdownlint.jsonc` は設定ファイルごと削除する（prek / CI / Makefile から参照されておらず、markdownlint フックも無いため）
- 到達不能な `_` アームは `unreachable!()` に置き換える。`target_os` の分岐統合は行わない
- `PixelBuffer::as_ptr` は公開 API として残し、`src/types.rs` の doc に用途と有効期間を明記する
- `PixelBuffer::from_retained_ptr` の `debug_assert!(ptr.is_null(), ...)` は `debug_assert!(false, ...)` に置き換える。デバッグビルドでの契約違反検出は維持する
- `FfiCaptureImpl` / `MfCaptureImpl` の `impl` ブロックは 1 つに統合する
- `CaptureOps` / `DeviceOps` のフィールドと `CaptureContext::callback` から `pub` を外し、`pub(crate)` 型のメソッドは `pub(crate)` に統一する
- `mjpeg_payload_bytes` は削除しない（private 関数の単体テストを PBT で代替できないため）
- `MfCaptureImpl::new` の `PixelFormat::Unknown` チェックは削除し、`process_sample` の `PixelFormat::Unknown(_) => return` アームは `match` の網羅性のため残す

## 完了条件

- 上記 1〜16 が設計方針に従って整理されている（現状維持の場合は理由が明記されている）
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `.github/workflows/ci.yml` の `CI` ジョブの全マトリクス（Ubuntu v4l2 / pipewire / v4l2,pipewire、Ubuntu ARM v4l2、macOS、Windows）が通る
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する
- 挙動が変わっていない

## 解決方法

{完了時に記入}
