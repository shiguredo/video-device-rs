# フレームコールバック内からの VideoCapture の drop で FFI バックエンドが UAF・デッドロックする

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-callback-internal-drop
- Polished: 2026-11-05

## 目的

`Arc<Mutex<Option<VideoCapture>>>` をコールバックに渡し、コールバック内で `take()` して drop するコードは safe Rust で書ける。このとき `FfiCaptureImpl::drop` が同じコールバック実行中に `stop()` とセッション破棄を行い、次の問題が起きる。

- V4L2: `video_v4l2_session_stop` の `pthread_join` が自分自身への join になり失敗するが、戻り値は無視され、`video_v4l2_session_destroy` が mmap / fd / `VideoSession` を解放する。コールバックから戻った `capture_thread` が解放済みの `VideoSession` を読む（use-after-free）
- PipeWire: `on_process` は PipeWire のデータループスレッド上で実行される（スレッドループとは別スレッド）。`video_pipewire_session_stop` はスレッドループのロックを取得して `pw_stream_disconnect` を呼び、`pw_thread_loop_stop` でスレッドループを join するため停止処理が進む。`video_pipewire_session_destroy` が stream / core / context / thread loop / `VideoSession` を破棄した後、データループ上でまだ実行中の `on_process` が戻った後に `pw_stream_queue_buffer` で解放済みの session と stream を触る（use-after-free）
- AVF: `video_avf_session_stop` の `dispatch_sync(session->queue, ...)` を同じ serial queue 上から呼ぶためデッドロックする
- Rust 側でも `Box<CaptureContext>`（実行中のクロージャ環境）を実行中に drop することになり、言語的にも未定義動作

README と crate doc (`src/lib.rs`) は「コールバック内から `stop()` を呼ばないこと」とだけ書いており、drop については触れていない。rustdoc の `VideoCapture::stop` (`src/capture.rs`) は「ブロッキング: 全バックエンドでキャプチャスレッド/コールバックの完了を待機してから復帰する」と書いているが、コールバック内から呼ぶとこの保証は成立しない。

## 優先度根拠

- High。safe Rust だけで到達できるメモリ安全性の破綻であり、ライブラリの安全性契約に関わる
- 実際に使われうるパターン（チャネルを介さず capture を共有する構成）で踏む
- `/review-code` の致命的指摘として確認済み
- 対象は FFI バックエンド（V4L2 / PipeWire / AVF）。Windows MF は `VideoCapture` が `!Send`（`src/types.rs` の `CoInitGuard` が `PhantomData<*const ()>` を保持する）ため、`Arc<Mutex<Option<VideoCapture>>>` を `Fn + Send + 'static` のコールバックへ渡せず、コールバック内 drop は起きない

## 現状

- `src/capture_ffi.rs` の `impl Drop for FfiCaptureImpl` は `self.stop()` の後に `session_destroy` を呼ぶ
- `src/video_v4l2.c` の `video_v4l2_session_stop` は `pthread_join(session->thread, NULL)` の戻り値を確認しない
- `src/video_pipewire.c` の `video_pipewire_session_destroy` は stream / core / context / thread loop / session を解放する
- `src/video_avf.m` の `video_avf_session_stop` は `dispatch_sync(session->queue, ^{})` を実行する
- `src/lib.rs` と `README.md` の注意書きは `stop()` の再入のみを対象にしており、「特に Windows ではデッドロック」と書いているが、AVF もデッドロックする
- `src/capture.rs` の `VideoCapture::stop` は「ブロッキング: 全バックエンドでキャプチャスレッド/コールバックの完了を待機してから復帰する」とだけ書いており、コールバック内から呼ぶ場合の再入制約が無い
- `src/types.rs` の `CoInitGuard` は `PhantomData<*const ()>` を保持するため `MfCaptureImpl` は `!Send`。Windows の `VideoCapture` は `!Send` であり、`Arc<Mutex<Option<VideoCapture>>>` を `Fn + Send + 'static` のコールバックへ渡せない

## 設計方針

### 1. コールバック実行中の drop を安全にする（本筋）

`frame_callback` の実行中かどうかを thread-local なフラグ等で記録し、`FfiCaptureImpl::drop` がその実行中に呼ばれた場合は `stop()` と `session_destroy` を呼ばず、C セッションと `Box<CaptureContext>` の**両方**を解放せず残す（リークを許容して UAF を避ける）。

- C セッションだけ残して `Box<CaptureContext>` を drop すると、キャプチャスレッドが `user_data` を解放済みメモリとして参照するため、両方を残すこと
- 判定は `FfiCaptureImpl::running` ではなく、コールバック実行中かどうかで行う。コールバック内で `stop()` を呼んだ直後の drop は `running` が false でも C 側スレッドがまだコールバックから戻っていないため、同じ UAF を踏む
- フラグは `frame_callback` がユーザーコールバックを呼ぶ区間だけ立て、戻ったら下ろす
- この方式ではキャプチャスレッドは動き続け、drop 後もコールバックが呼ばれ続ける。C セッション（V4L2 の fd / mmap、PipeWire の stream / context、AVF のセッション）もプロセス終了まで解放されない

### 2. 代替案: 破棄を別スレッドへ移す（任意）

リークを避けたい場合は、`FfiCaptureImpl::drop` から C セッションと `Box<CaptureContext>` を専用スレッドへ move し、そのスレッドで `stop()` と `session_destroy` を実行してから `CaptureContext` を drop する。キャプチャスレッドはコールバックから戻った後に停止し、リークもコールバック継続も起きない。`NonNull<VideoSession>` は `!Send` なので、C 側 API が任意スレッドから呼べることを根拠に `unsafe impl Send` のラッパーで包む。どちらの方式を選んでもよいが、下記の完了条件は両方式で満たすこと。

### 3. 公開 doc の更新

- `stop()` はコールバック内から呼んではならない。AVF は `dispatch_sync` でデッドロックする。V4L2 は `pthread_join` が自分自身への join になり待機しない。PipeWire は `on_process` がデータループ上で動くため、`pw_thread_loop_stop` はスレッドループだけを待ち、実行中のコールバックの完了は待たない
- `drop` は修正後はコールバック内から呼んでも安全だが、1 の方式ではセッションが解放されずコールバックが呼ばれ続ける。可能ならコールバック外で drop すること
- 更新対象: `src/lib.rs`、`README.md`、`src/capture.rs` の `VideoCapture` と `VideoCapture::stop` の doc

## 完了条件

- `tests/test_capture.rs` に、コールバック内で `VideoCapture` を drop するテストを追加する
  - 既存の `device_test_enabled()` でガードし、`VIDEO_DEVICE_DEVICE_TEST` 未設定時はスキップする
  - `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` でテスト関数をガードする。Windows MF では `VideoCapture` が `!Send` のため、`Arc<Mutex<Option<VideoCapture>>>` をコールバックへ渡せずコンパイルできない
  - `Arc<Mutex<Option<VideoCapture>>>` をコールバックに渡し、指定フレーム数に達したらコールバック内で `take()` して drop する
  - テストがハングせず、drop 後に UAF でクラッシュしないこと。リーク方式を選んだ場合、drop 後もコールバックが呼ばれ続けることを許容し、コールバック側は `Option` が `None` でも panic しないこと
- Linux の device-test ランナーで V4L2 と PipeWire の両方を実行する。CI は `VIDEO_DEVICE_DEVICE_TEST=1` で `cargo test --workspace` と `cargo test --workspace --no-default-features --features default-pipewire` を実行する
- AVF は macOS の CI では実行されない（`.github/workflows/ci.yml` の device-test ジョブは `if: runner.os != 'macOS'` でキャプチャテストをスキップする。TCC のため）。macOS 実機で手動確認する
- ASAN 有効ビルドで use-after-free が報告されないこと。リポジトリに ASAN 用の CI / Makefile ターゲットは無いため手動で確認する。C 側を `-fsanitize=address` でビルドして実行する。リーク方式を選んだ場合、意図的なリークが LeakSanitizer に報告されうるため `ASAN_OPTIONS=detect_leaks=0` を付けるか、リーク報告を既知のものとして扱う
- `src/lib.rs`、`README.md`、`src/capture.rs` の `VideoCapture` と `VideoCapture::stop` に、`stop()` / drop の再入制約を明記する
- `cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る

## 解決方法

{完了時に記入}
