# PipeWire のストリームネゴシエーションで未知フォーマットと範囲外要求が無音で失敗する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-negotiation-robustness
- Polished: 2026-11-05

## 目的

`src/video_pipewire.c` の `video_pipewire_session_start` と `on_param_changed` / `on_process` に、ネゴシエーション失敗が利用者に伝わらない経路が 2 つある。

1. 要求した幅・高さ・フレームレートが choice の固定上限（幅 3840、高さ 2160、fps 60）を超えると、`def_size` / `def_fps` が choice の範囲外になる。サーバは範囲内へクランプするか既定値を無視するため、要求と違う解像度・フレームレートがエラーなしで返る。この上限は README / docs/LINUX.md / クレート doc には書かれていない（`skills/shiguredo-video-device/SKILL.md` には記載がある）
2. raw 形式でネゴシエーション結果が NV12 / YUY2 / I420 以外の場合、`on_param_changed` は `format_ready` を 1 にし、`on_process` は毎回バッファをキューへ戻すだけになる。`start()` は成功を返し、フレームは 1 枚も届かず、ログも状態も残らない。現行の `SPA_POD_CHOICE_ENUM_Id(4, ...)` は NV12 / YUY2 / I420 に限定しているため通常は到達しないが、choice の変更やサーバが想定外の raw フォーマットを返した場合に備えた防御として扱う

## 優先度根拠

- Medium。いずれも「成功したように見えて要求どおりに動かない」無音故障
- 4K を超える解像度や 60fps 超を要求するカメラ・利用者で再現する
- 未知フォーマットは現行の choice では通常到達しないが、コード上は無音でフレームが落ちる経路であり、防御として修正する
- `/review-code` の指摘として確認済み

## 現状

```c
struct spa_fraction def_fps = SPA_FRACTION(session->requested_fps, 1);
struct spa_fraction max_fps = SPA_FRACTION(60, 1);
struct spa_rectangle def_size = SPA_RECTANGLE(session->requested_width, session->requested_height);
struct spa_rectangle max_size = SPA_RECTANGLE(3840, 2160);
```

`convert_spa_video_format` は NV12 / YUY2 / I420 以外で 0 を返し、`on_process` の `else` 分岐でバッファを戻すだけになる。`on_param_changed` は `format_ready` を 1 にするため、`on_process` はバッファをキューへ戻すだけの処理を繰り返す。

`video_pipewire_session_start` の待機ループは `PW_STREAM_STATE_STREAMING` を先に判定して抜けるため、`on_param_changed` で `stream_error` を立てるだけでは `start()` は成功を返す。

## 設計方針

- 上限超過（幅 3840 超、高さ 2160 超、fps 60 超）は `src/capture_ffi.rs` の `FfiCaptureImpl::new_pipewire` で `Error::InvalidCaptureConfig` として事前に拒否する。`FfiCaptureImpl::new` に置くと AVF / V4L2 にも上限が適用されるため、PipeWire バックエンドの構築関数で判定する（0032 の正値バリデーションとは独立）
- choice の上限を要求値以上に動的に広げる案は、デバイスが要求値をサポートしない場合に要求と異なる値が無音で返るため、本 issue では採用しない。固定上限を PipeWire の仕様として維持する
- `on_param_changed` で `convert_spa_video_format` が 0 を返した場合、`format_ready` を立てずに `stream_error` を立てて `pw_thread_loop_signal` する。`video_pipewire_session_start` の待機ループで `stream_error` の判定を `PW_STREAM_STATE_STREAMING` より前に移し、`start()` が `-5` を返すようにする。初回のみ stderr に英語のログを出す
- 上限値（3840x2160 / 60fps）と超過時のエラーを README / docs/LINUX.md / `src/lib.rs`（クレート doc） / `VideoCaptureConfig` / `VideoCapture::new_pipewire` の doc に明記する。`skills/shiguredo-video-device/SKILL.md` は既に上限を記載しているが、超過時はエラーになる旨に更新する
- 変更対象は `src/capture_ffi.rs` の `FfiCaptureImpl::new_pipewire`、`src/video_pipewire.c` の `on_param_changed` / `on_process` / `video_pipewire_session_start`、`src/capture.rs` の `VideoCapture::new_pipewire` doc、`tests/test_capture.rs`、`README.md` / `docs/LINUX.md` / `src/types.rs` / `src/lib.rs` / `skills/shiguredo-video-device/SKILL.md`

## 完了条件

- 要求値が現在の固定上限（幅 3840、高さ 2160、fps 60）を超える場合、`Error::InvalidCaptureConfig` を返す（無音のクランプが起きない）
- 未知フォーマットが選ばれた場合、`start()` が `Err` を返す（ログも出す）
- 既存の NV12 / YUY2 / I420 の経路が変わらない
- 上限超過の拒否は `tests/test_capture.rs` のデバイス不要テストで固定する
- Linux 実機の device test が通る
- `cargo build --workspace --features pipewire` / `cargo clippy --workspace --all-targets --features pipewire -- -D warnings` / `cargo test --workspace --features pipewire` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
