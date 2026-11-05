# AVF のセッション実行時エラー・中断・デバイス切断が検知できない

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-runtime-error-observability
- Polished: 2026-11-05

## 目的

`src/video_avf.m` は `AVCaptureSessionRuntimeErrorNotification` と `AVCaptureSessionWasInterruptedNotification` を購読していない。キャプチャ開始後に実行時エラー、システムによる中断、カメラの切断が起きると、`captureOutput` が呼ばれなくなる。AVFoundation はこれらの状態変化を通知するが、`src/video_avf.m` は購読していないため、Rust 側の `FfiCaptureImpl::running` は `true` のままになる。利用者が `start()` を呼んでも `FfiCaptureImpl::start` は C を呼ばずに `Ok(())` を返し、`stop()` は `()` を返すだけで停止理由を伝えず、コールバックも呼ばれない。利用者からは「フレームが止まった」ことしか分からない。

## 優先度根拠

- Medium。キャプチャ開始後の障害が一切観測できず、アプリケーション側で復帰処理が書けない
- macOS の実キャプチャは CI で実行されない。`.github/workflows/ci.yml` の device-test ジョブは macOS で `VIDEO_DEVICE_DEVICE_TEST` を設定せず、`tests/test_capture.rs` のキャプチャテストはスキップされる。この経路は自動検証もされていない
- `/review-code` の重要指摘として確認済み

## 現状

- `src/video_avf.m` 内に `NSNotificationCenter` の使用箇所が無く、`AVCaptureSessionRuntimeErrorNotification` と `AVCaptureSessionWasInterruptedNotification` を購読していない
- `struct VideoSession` は `session` / `input` / `output` / `delegate` / `queue` だけを保持し、通知の observer を保持していない
- `video_avf_session_start` は `startRunning` の直後に `session.running` を確認するだけで、開始後の状態変化を監視していない。`startRunning` 自体の失敗はこの確認で検出できるが、開始後の実行時エラー・中断・切断は検出できない
- `src/capture_ffi.rs` の `FfiCaptureImpl::start` は `self.running` が真なら C の `video_avf_session_start` を呼ばずに `Ok(())` を返す
- `video_avf_session_destroy` は通知の observer を解除する処理を持たない

## 設計方針

- `struct VideoSession` に、`atomic_int faulted` を保持する専用の ObjC オブジェクト（例: `VideoSessionObserver`）と、通知 observer の token を保持する `id` フィールドを追加する。専用オブジェクトは `struct VideoSession` への参照を持たない。`faulted` は専用オブジェクトの `init` で `atomic_init` する
- `video_avf_session_create` の成功直前（全フィールドを設定した後）に、`object:session->session`、`queue:nil` で `AVCaptureSessionRuntimeErrorNotification` と `AVCaptureSessionWasInterruptedNotification` の observer を登録する。ブロックは専用の ObjC オブジェクトを捕捉し、`atomic_store` で `faulted` を立てる。runtime error では `userInfo[AVCaptureSessionErrorKey]` の error code / description、interruption では `AVCaptureSessionInterruptionReasonKey` を英語で `NSLog` する。ブロックは `struct VideoSession` を捕捉しない
- `video_avf_session_start` は `faulted` が真なら負の値（-2）を返す。`video_avf_session_stop` では `faulted` を解除しない（新しい `VideoCapture` を作り直すまで再 `start` 不能とする）
- `video_avf_session_destroy` では observer を `removeObserver:` で解除し、token と専用オブジェクトを nil にしてから既存の破棄処理を行う。`video_avf_session_create` の失敗経路で observer を登録した場合は解除する（登録を成功直前に行えば失敗経路は無いが、防御的に）
- `video_avf_session_faulted` を追加し、`src/video_avf.h` に宣言する。`src/capture_ffi.rs` の `CaptureOps` に faulted 照会関数（`Option`）を追加し、AVF は `Some(video_avf_session_faulted)`、V4L2 / PipeWire は `None` にする。既に同等の field がある場合はそれを利用する
- `FfiCaptureImpl::start` は `self.running` の早期 return より前に faulted を照会し、真なら `Error::CaptureFaulted` を返す。`self.running` は変更しない（`stop()` / `Drop` が C の `session_stop` を呼んで delegate 解除と queue の drain を行うため）
- 非同期のエラーコールバックや新しい公開 API は追加しない。`Error::CaptureFaulted` の doc / `Display`、`VideoCapture::start` の doc、README、`skills/shiguredo-video-device/SKILL.md` を AVF の実行時エラー・中断・切断にも対応した表現に更新する
- `AVCaptureSessionInterruptionEndedNotification` は購読しない。中断終了後の自動再開には依存せず、faulted は `stop()` では解除しない

## 変更対象

- `src/video_avf.m`: `faulted` 状態、通知 observer の登録・解除、runtime error / interruption の処理
- `src/video_avf.h`: `video_avf_session_faulted` の宣言
- `src/capture_ffi.rs`: `CaptureOps` の faulted 照会、`FfiCaptureImpl::start` の faulted チェックと doc 更新
- `src/error.rs`, `src/capture.rs`, `src/lib.rs`: `Error::CaptureFaulted` と `VideoCapture::start` の doc / `Display`
- `README.md`, `skills/shiguredo-video-device/SKILL.md`: macOS の faulted 挙動
- `CHANGES.md`: `## develop` に `[FIX]` エントリ
- `tests/test_capture.rs`: macOS 実機用の手動テスト

## 完了条件

- 通知を受信した後の次の `start()` が、`stop()` の有無にかかわらず `Error::CaptureFaulted` を返す
- `stop()` では faulted は解除されず、利用者は新しい `VideoCapture` を作り直す必要がある（doc に明記する）
- 通知の登録・解除が対になっており、`video_avf_session_destroy` 後に observer が残らない（コードレビューで確認）
- 通知ブロックは `struct VideoSession` を捕捉せず、`destroy` 後の in-flight ブロックが C 構造体を参照しない（コードレビューで確認）
- 通知受信時に runtime error の error code / interruption の reason を含む英語のログが出る
- 既存の正常系キャプチャが変わらない（macOS 実機で通常のフレーム受信が継続する）
- `Error::CaptureFaulted` の doc / `Display`、`VideoCapture::start` の doc、`FfiCaptureImpl::start` の doc、README、`skills/shiguredo-video-device/SKILL.md` が AVF の挙動と一致する
- `CHANGES.md` の `## develop` に `[FIX]` エントリが追加されている
- macOS で `cargo fmt --all --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` が通る
- macOS 実機で確認できる手順が issue に残っている（下記「確認手順」）

## 確認手順

1. 外部 USB カメラを接続する
2. `tests/test_capture.rs` に `test_capture_runtime_error` を追加する。`VIDEO_DEVICE_DEVICE_TEST` と `VIDEO_DEVICE_DISCONNECT_TEST` が設定されているときだけ実行し、外部カメラの `unique_id` を `VIDEO_DEVICE_ID` で指定する。macOS (AVF) でのみ実行し、CI では `VIDEO_DEVICE_DISCONNECT_TEST` を設定しないため実行されない
3. テストは `start()` 後に数フレーム受信し、「カメラを抜いてください」と表示して 10 秒待つ。その間に USB カメラを抜く
4. テストは `stop()` を呼んでから `start()` を呼び、`Err(Error::CaptureFaulted)` を確認する
5. 通常系は `cargo run --example camera_preview` でフレームが表示されることを確認する
6. `AVCaptureSessionWasInterruptedNotification` は macOS 実機で意図的に発生させにくいため、observer が登録され、runtime error と同じ `faulted` 処理を行うことをコードレビューで確認する。可能ならスリープ / 復帰などで確認する

## スコープ外

- 非同期のエラーコールバックや新しい公開 API の追加。既存の `Error::CaptureFaulted` で通知する
- `AVCaptureSessionInterruptionEndedNotification` の購読と、中断終了後の自動再開。faulted は `stop()` では解除せず、新しい `VideoCapture` を作り直す
- `AVCaptureSessionDidStopRunningNotification` の購読。正常停止と区別できないため、runtime error と interruption だけを扱う

## 解決方法

{完了時に記入}
