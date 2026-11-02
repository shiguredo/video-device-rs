# フレームコールバック内からの VideoCapture の drop で FFI バックエンドが UAF・デッドロックする

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-callback-internal-drop
- Polished: {YYYY-MM-DD}

## 目的

`Arc<Mutex<Option<VideoCapture>>>` をコールバックに渡し、コールバック内で `take()` して drop するコードは safe Rust で書ける。このとき `FfiCaptureImpl::drop` が同じコールバック実行中に `stop()` とセッション破棄を行い、次の問題が起きる。

- V4L2: `video_v4l2_session_stop` の `pthread_join` が自分自身への join になり失敗するが、戻り値は無視され、`video_v4l2_session_destroy` が mmap / fd / `VideoSession` を解放する。コールバックから戻った `capture_thread` が解放済みの `VideoSession` を読む（use-after-free）
- PipeWire: 再帰ロックのため停止処理が進み、`video_pipewire_session_destroy` が stream / context / thread loop を破棄した後に `on_process` の `pw_stream_queue_buffer` が解放済み session を触る（use-after-free）
- AVF: `video_avf_session_stop` の `dispatch_sync(session->queue, ...)` を同じ serial queue 上から呼ぶためデッドロックする
- Rust 側でも `Box<CaptureContext>`（実行中のクロージャ環境）を実行中に drop することになり、言語的にも未定義動作

公開 doc は「コールバック内から `stop()` を呼ばないこと」とだけ書いており、drop については触れていない。

## 優先度根拠

- High。safe Rust だけで到達できるメモリ安全性の破綻であり、ライブラリの安全性契約に関わる
- 実際に使われうるパターン（チャネルを介さず capture を共有する構成）で踏む
- `/review-code` の致命的指摘として確認済み

## 現状

- `src/capture_ffi.rs` の `impl Drop for FfiCaptureImpl` は `self.stop()` の後に `session_destroy` を呼ぶ
- `src/video_v4l2.c` の `video_v4l2_session_stop` は `pthread_join(session->thread, NULL)` の戻り値を確認しない
- `src/video_pipewire.c` の `video_pipewire_session_destroy` は stream / context / thread loop / session を解放する
- `src/video_avf.m` の `video_avf_session_stop` は `dispatch_sync(session->queue, ^{})` を実行する
- `src/lib.rs` / `README.md` の注意書きは `stop()` の再入のみを対象にしている

## 設計方針

- `frame_callback` の実行中かどうかを thread-local なフラグ等で記録し、実行中の drop では停止・破棄を遅延させる（リークを許容して UAF を避ける）
- 少なくとも全バックエンドについて「コールバック内から `stop()` と drop の両方を呼んではならない」と doc に明記する
- 可能なら、コールバック実行中に破棄要求が来たことを検出して安全に停止できる設計（別スレッドでの破棄など）を検討する

## 完了条件

- コールバック内で `VideoCapture` を drop するテストを追加し、V4L2 / PipeWire / AVF で UAF やデッドロックが発生しない（または明示的なエラーになる）
- ASAN 有効ビルドで use-after-free が検出されない
- `VideoCapture` と `stop()` / drop の再入制約が公開 doc に明記されている

## 解決方法

{完了時に記入}
