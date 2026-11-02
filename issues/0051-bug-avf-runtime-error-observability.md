# AVF のセッション実行時エラー・中断・デバイス切断が検知できない

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-runtime-error-observability
- Polished: {YYYY-MM-DD}

## 目的

`src/video_avf.m` は `AVCaptureSessionRuntimeErrorNotification` と `AVCaptureSessionWasInterruptedNotification` を購読していない。実行時エラー、システムによる中断、カメラの切断が起きた場合、`captureOutput` が呼ばれなくなるだけで、`VideoSession` は動作中のまま、`start()` / `stop()` はエラーを返さず、コールバックも呼ばれない。利用者からは「フレームが止まった」ことしか分からない。

## 優先度根拠

- Medium。キャプチャ開始後の障害が一切観測できず、アプリケーション側で復帰処理が書けない
- macOS の実キャプチャは CI で実行されないため、この経路は自動検証もされていない
- `/review-code` の重要指摘として確認済み

## 現状

`src/video_avf.m` 内に `NSNotificationCenter` の使用箇所が無く、セッションの runtime error / interruption 通知を購読していない。`video_avf_session_start` は起動直後の `session.running` しか確認しない。

## 設計方針

- セッション生成時に通知を登録し、エラー・中断を検知したら停止状態へ遷移させる。少なくとも次の `start()` が失敗するようにする
- 利用者へ通知する API（エラーコールバックなど）を追加するか、既存の `Error` で表現できる経路を用意する。API 追加は別 issue として分割してもよい
- 通知の解除を `video_avf_session_destroy` で確実に行う

## 完了条件

- セッション実行時エラー・中断・切断を発生させたとき、`stop()` 後の再 `start()` が失敗するか、利用者へエラーが通知される
- 通知の登録・解除がリークしない
- 既存の正常系キャプチャが変わらない
- macOS 実機で確認できる手順が issue に残っている

## 解決方法

{完了時に記入}
