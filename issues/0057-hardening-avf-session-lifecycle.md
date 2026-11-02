# AVF のセッション生成・破棄経路と Objective-C メモリ管理を堅牢化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-session-lifecycle-hardening
- Polished: {YYYY-MM-DD}

## 目的

`src/video_avf.m` に、Objective-C のメモリ管理とセッション寿命に関する不備が複数ある。

1. C 入口（`video_avf_enumerate_devices` / `video_avf_session_create` / `video_avf_session_start` / `video_avf_session_stop` / delegate の `captureOutput`）に `@autoreleasepool` が無い。Rust が作るスレッドや独自 dispatch queue には自動解放プールが無いため、`NSString` / `NSError` / discovery session の結果などがスレッド終了まで滞留する
2. `CMTimeGetSeconds(pts) * 1000000.0` をそのまま `int64_t` へキャストしている。PTS が無効値だと NaN になり、NaN から整数への変換は C の未定義動作
3. `video_avf_session_start` が `startRunning` の後に `session.running` を確認して -2 を返す経路があるが、`VideoCapture` をそのまま drop すると `video_avf_session_stop` を経由せずに `video_avf_session_destroy` が呼ばれ、delegate と queue を即座に解放する。コールバックが飛んでいる可能性を排除できない
4. I420 の詰め替えで `chromaHeight = (height + 1) / 2` としているため、奇数高さのとき最終行を余分に `memcpy` しうる
5. `calloc` / `free` / `strdup` / `memcpy` / `abs` / `INT_MAX` / `fabs` を使うが、`<stdlib.h>` / `<string.h>` / `<limits.h>` / `<math.h>` を明示的に include していない（Apple フレームワークの推移的 include に依存）
6. `video_avf_enumerate_devices` は `frameRateRanges[0]` で初期化した後に全要素を走査するため先頭要素を二重処理する。また `formatArray` は `formatCount` 分確保したまま「後で実際の数に調整」とコメントしているが縮小していない

## 優先度根拠

- Medium。いずれも通常は踏まないが、ヘッドレス環境・異常な PTS・start 失敗時の破棄で問題になりうる
- 1 と 5 は可読性・移植性、2 は未定義動作、3 と 4 は破棄経路の安全性
- `/review-code` の指摘として確認済み

## 現状

- `src/video_avf.m` に `@autoreleasepool` の記述が無い
- `int64_t timestamp_us = (int64_t)(CMTimeGetSeconds(pts) * 1000000.0);` としている
- `video_avf_session_stop` のみが `dispatch_sync` でコールバック完了を待つ

## 設計方針

- 各 C 入口を `@autoreleasepool { ... }` で囲む
- `CMTIME_IS_VALID(pts)` を確認し、無効時は 0 または単調時計にフォールバックする
- `CMTIME_IS_NUMERIC` 相当のチェックを追加する
- 破棄時に delegate 解除と queue の同期を行う経路を統一する
- 420 は偶数高さ前提であることを検証するか、奇数高さを切り捨てる
- 使用する標準ヘッダを明示的に include する

## 完了条件

- ヘッドレス環境で列挙・生成を繰り返しても Objective-C オブジェクトが滞留しない
- 無効な PTS でも未定義動作にならない
- start 失敗後の drop でコールバックとの競合が起きない
- 標準ヘッダの include が明示されている
- macOS 実機でキャプチャが動作する

## 解決方法

{完了時に記入}
