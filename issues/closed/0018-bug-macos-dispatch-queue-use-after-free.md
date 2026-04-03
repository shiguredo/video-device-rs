# macOS 停止時に dispatch queue の in-flight callback が use-after-free を起こしうる

Created: 2026-04-03
Model: Claude Opus 4.6

## 概要

macOS で `VideoCapture` を停止・破棄する際、`dispatch_queue_t` 上で既に実行中またはキューに載っているコールバックの完了を待たずに teardown が進むため、`CaptureContext` の解放後にコールバックが `userData` を参照して use-after-free が発生しうる。

## 根拠

`AVCaptureVideoDataOutput` の `setSampleBufferDelegate:nil` は新規ディスパッチを止めるが、Apple のドキュメントによれば **既にキューに入っているコールバックは実行が完了するまで走りうる**。現在の実装では:

1. `video_session_stop` (`src/video_c.m:494-501`) が `setSampleBufferDelegate:nil` → `stopRunning` を呼ぶ
2. しかし dispatch queue 上の in-flight コールバック完了を待つ同期処理がない
3. `Drop` (`src/capture.rs:97-104`) が `stop()` の直後に `video_session_destroy` を呼び、`free(session)` で session 構造体を解放する
4. その後 `Arc<CaptureContext>` が drop される
5. 手順 2 で残っていたコールバックが `userData` (= `CaptureContext` への生ポインタ) を触るとクラッシュ

対照的に、V4L2 (Linux) は `pthread_join` でキャプチャスレッドの終了を待っており、Windows は `handle.join()` でスレッド完了を同期している。macOS だけ同期が欠けている。

## 再現条件

- macOS 環境で `VideoCapture` の `start` → `stop` (または `drop`) を高速に繰り返す
- delegate queue 上のコールバック処理が重い状態で停止する
- タイミング依存のため通常使用では発現しにくいが、負荷が高い場合やマルチコア環境で顕在化しうる

## 対策案

`video_session_stop` の `setSampleBufferDelegate:nil` の直後に `dispatch_sync(session->queue, ^{})` を挿入し、キュー上の既存処理を drain する。

```objc
void video_session_stop(struct VideoSession* session) {
    if (!session) {
        return;
    }

    [session->output setSampleBufferDelegate:nil queue:nil];

    // キュー上の in-flight コールバックの完了を待つ
    dispatch_sync(session->queue, ^{});

    [session->session stopRunning];
}
```

これにより `dispatch_sync` が返った時点でキュー上の全コールバックが完了しており、以降 `userData` へのアクセスは発生しない。

## 影響範囲

- `src/video_c.m`: `video_session_stop` 関数
- macOS のみ (Linux / Windows は影響なし)

## 解決方法

Completed: 2026-04-03

`video_session_stop` の `setSampleBufferDelegate:nil` の直後に `dispatch_sync(session->queue, ^{})` を追加した。`session->queue` の NULL チェック付き。これにより delegate 解除後、キュー上の in-flight コールバックが全て完了してから `stopRunning` および後続の teardown に進むことが保証される。
