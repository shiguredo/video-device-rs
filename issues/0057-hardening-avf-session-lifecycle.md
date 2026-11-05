# AVF のセッションライフサイクル・メモリ管理・フレーム詰め替え・列挙を堅牢化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-session-lifecycle-hardening
- Polished: 2026-11-05

## 目的

`src/video_avf.m` に、Objective-C のメモリ管理、セッション寿命、フレーム詰め替え、デバイス列挙に関する不備が複数ある。

1. C 関数と delegate メソッド（`video_avf_enumerate_devices` / `video_avf_session_create` / `video_avf_session_start` / `video_avf_session_stop` / `video_avf_session_destroy` / `captureOutput:didOutputSampleBuffer:fromConnection:`）に `@autoreleasepool` が無い。Rust が作るスレッドや独自 dispatch queue には自動解放プールが無いため、`NSString` / `NSError` / discovery session の結果などがスレッド終了まで滞留する
2. `CMTimeGetSeconds(pts) * 1000000.0` をそのまま `int64_t` へキャストしている。PTS が無効値・不定値だと NaN、±無限大だと ±inf になり、NaN / ±inf から整数への変換は C の未定義動作
3. `video_avf_session_start` が `startRunning` の後に `session.running` を確認して -2 を返す経路がある。このとき `src/capture_ffi.rs` の `FfiCaptureImpl::start` は `running` を true にしないため、`Drop` の `FfiCaptureImpl::stop` は C の `video_avf_session_stop` を呼ばず、`video_avf_session_destroy` だけが呼ばれる。`video_avf_session_destroy` は delegate を外さず queue の drain もしないため、コールバックが飛んでいる可能性を排除できない
4. I420 の詰め替えで `chromaHeight = (height + 1) / 2` を U/V プレーンの実高さとして扱っている。`CVPixelBufferGetHeightOfPlane` と照合していないため、奇数高さでプレーンの実高さが小さい場合、最終行の `memcpy` が範囲外読み出しになる
5. `calloc` / `free` / `strdup` / `memcpy` / `abs` / `INT_MAX` / `fabs` を使うが、`<stdlib.h>` / `<string.h>` / `<limits.h>` / `<math.h>` を明示的に include していない（Apple フレームワークの推移的 include に依存）
6. `video_avf_enumerate_devices` は `frameRateRanges[0]` で初期化した後に全要素を走査するため先頭要素を二重処理する。また `formatArray` は `formatCount` 分確保したまま「後で実際の数に調整」とコメントしているが縮小していない

## 優先度根拠

- Medium。いずれも通常は踏まないが、ヘッドレス環境・異常な PTS・start 失敗時の破棄で問題になりうる
- 1 はメモリ解放漏れ、2 は未定義動作、3 は破棄経路の競合、4 は範囲外読み出し、5 は移植性、6 は列挙の冗長処理とメモリ浪費
- `/review-code` の指摘として確認済み

## 現状

- `src/video_avf.m` に `@autoreleasepool` の記述が無い
- `int64_t timestamp_us = (int64_t)(CMTimeGetSeconds(pts) * 1000000.0);` としている
- `video_avf_session_stop` のみが `setSampleBufferDelegate:nil` と `dispatch_sync` でコールバック完了を待つ。`video_avf_session_destroy` はどちらも行わない
- I420 の詰め替えは `size_t chromaHeight = (height + 1) / 2;` とし、`for (size_t row = 0; row < chromaHeight; row++)` で `memcpy` する。`CVPixelBufferGetHeightOfPlane` は使っていない
- include は Apple フレームワークと `video_avf.h` のみで、`<stdlib.h>` / `<string.h>` / `<limits.h>` / `<math.h>` が無い
- `video_avf_enumerate_devices` は `AVFrameRateRange* range = frameRateRanges[0];` で初期化した後、`for (AVFrameRateRange* r in frameRateRanges)` で先頭要素も再処理する。`formatArray` は `formatCount` 分を `malloc` し、`actualCount` に縮小しない

## 設計方針

- `video_avf_enumerate_devices` / `video_avf_session_create` / `video_avf_session_start` / `video_avf_session_stop` / `video_avf_session_destroy` の各 C 関数と `captureOutput:didOutputSampleBuffer:fromConnection:` を `@autoreleasepool { ... }` で囲む。`video_avf_free_devices` と `video_avf_device_*` は ObjC オブジェクトを生成しないため対象外とする
- `CMTIME_IS_NUMERIC(pts)` を確認し、偽なら `timestamp_us = 0` とする。`CMTIME_IS_VALID` だけでは indefinite / ±infinity を除外できないため、`CMTIME_IS_NUMERIC` を使う。`0` は不明値として扱う
- `video_avf_session_destroy` は `video_avf_session_stop` を無条件に呼んでから input / output を外す。`video_avf_session_start` が -2 を返す場合も、戻る前に `video_avf_session_stop` を呼ぶ。`video_avf_session_stop` は既に delegate 解除と `dispatch_sync` を行うため、この 1 経路に統一する。`video_avf_session_stop` は冪等であり、二重呼び出しを許容する
- I420 は `CVPixelBufferGetHeightOfPlane(imageBuffer, 1)` と `CVPixelBufferGetHeightOfPlane(imageBuffer, 2)` で U/V プレーンの実高さを取得し、コピー行数を U は `min((height + 1) / 2, planeHeightU)`、V は `min((height + 1) / 2, planeHeightV)` に制限する。`uvSize` は Rust `frame_math::i420_plane_sizes` の `stride_uv * ((height + 1) / 2) * 2` と一致させ、`calloc` のゼロ初期化で不足行を 0 として扱う。`CVPixelBufferGetHeightOfPlane` が 0 を返す場合はそのフレームを破棄する。`src/frame_math.rs` の `i420_plane_sizes` の doc とテストコメントにある `video_c.m` は `video_avf.m` に更新する（式は変更しない）
- `#include <stdlib.h>` / `#include <string.h>` / `#include <limits.h>` / `#include <math.h>` を追加する
- `video_avf_enumerate_devices` は `frameRateRanges[0]` で初期化した後、2 番目以降だけを走査する。`formatArray` は `actualCount > 0` の場合は `actualCount` に `realloc` で縮小し、`actualCount == 0` の場合は既存どおり `free` する。`realloc` 失敗時は元の配列をそのまま使う（`realloc` 失敗時は元のポインタが有効）。「後で実際の数に調整」のコメントを実装に合わせる

## 変更対象

- `src/video_avf.m`: 上記 1〜6 の修正
- `src/frame_math.rs`: `i420_plane_sizes` の doc とテストコメントのファイル名を `video_avf.m` に更新する（コメントのみ）
- `CHANGES.md`: `## develop` に `[FIX]` エントリを追加する（挙動に影響しない整理は `### misc`）

## 完了条件

- `@autoreleasepool` が列挙・生成・start・stop・destroy・`captureOutput` の各入口にあり、Instruments の Leaks で列挙・生成・破棄を繰り返した後にリークが増えないことを確認する（CI では実行しない）
- 無効な PTS のフレームは `timestamp_us = 0` で渡され、NaN / ±inf の整数変換が起きない
- start 失敗時も destroy 時に `video_avf_session_stop` が呼ばれ、delegate 解除と queue drain が行われる。start 失敗後にコールバックが呼ばれない
- 奇数高さの I420 でも `CVPixelBufferGetHeightOfPlane` を超える行を `memcpy` しない（`min` で制限する）。`uvSize` は Rust `i420_plane_sizes` と一致する
- `<stdlib.h>` / `<string.h>` / `<limits.h>` / `<math.h>` が明示的に include されている
- `frameRateRanges` の先頭要素を二重処理しない。`formatArray` が `actualCount` 分に縮小される（`actualCount == 0` の場合は `free`、失敗時は元の配列を使う）
- macOS で `cargo build --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通る
- macOS 実機で、対応するカメラの NV12 / YUY2 / I420 のキャプチャが動作する
- `CHANGES.md` の `## develop` に `[FIX]` エントリが追加されている

## スコープ外

- `issues/0035-bug-avf-error-path-hardening.md`: `captureOutput` の `CVPixelBufferLockBaseAddress` 戻り値確認と `video_avf_enumerate_devices` の `UTF8String` NULL
- `issues/0049-bug-avf-device-selection.md`: `video_avf_session_create` の device_id / pixel_format と `video_avf_enumerate_devices` の deviceTypes
- `issues/0050-bug-avf-uyvy-mislabel.md`: UYVY ('2vuy') の扱い
- `issues/0051-bug-avf-runtime-error-observability.md`: `video_avf_session_start` の faulted チェックと `video_avf_session_destroy` の observer 解除
- `issues/0052-bug-ffi-error-mapping.md`: `video_avf_session_create` の `error_code` 出力引数と `video_avf_enumerate_devices` のエラーコード
- `issues/0056-hardening-ffi-boundary-defense.md`: `video_avf_enumerate_devices` の `strdup` 戻り値確認

これらの issue は `src/video_avf.m` の同じ関数を変更する。先に実装する側が、後から実装する側の変更を取り込めるよう、関数のシグネチャと失敗経路の扱いを揃える。本 issue は上記の各項目を対象外とし、重複して修正しない。

## 解決方法

{完了時に記入}
