# Linux (V4L2 / PipeWire) と Windows (MF) で UYVY を列挙・キャプチャできるようにする

- Priority: Low
- Created: 2026-11-05
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/add-uyvy-linux-windows
- Polished: {YYYY-MM-DD}

## 目的

`issues/0050-bug-avf-uyvy-mislabel.md` で `PixelFormat::Uyvy` を追加するが、AVF のみ対応のままでは Linux (V4L2 / PipeWire) と Windows (MF) の利用者が UYVY を扱えない。各バックエンドで UYVY を列挙・キャプチャできるようにする。

## 現状

`src/video_v4l2.c` の `convert_v4l2_pixel_format` と `convert_video_pixel_format_to_v4l2` は NV12 / YUY2 / I420 / MJPEG のみを変換し、`V4L2_PIX_FMT_UYVY` を扱わない。`capture_thread` も UYVY の分岐を持たない。

`src/video_pipewire.c` の `convert_spa_video_format` と `convert_video_pixel_format_to_spa` は NV12 / YUY2 / I420 のみを変換し、`SPA_VIDEO_FORMAT_UYVY` を扱わない。`on_param_changed` の choice と `on_process` の分岐にも UYVY がない。

`src/capture_mf.rs` は `PixelFormat::Yuy2` の分岐を持つが `PixelFormat::Uyvy` の分岐を持たない。`src/types.rs` の `guid_to_pixel_format` / `pixel_format_to_guid` も `MFVideoFormat_UYVY` を扱わない。

`src/capture_ffi.rs` の `frame_callback` には 0050 で `PixelFormat::Uyvy` の分岐が追加されるが、C 側が `VIDEO_PIXEL_FORMAT_UYVY` を返さないため到達しない。

## 設計方針

- `src/video_v4l2.c`: `convert_v4l2_pixel_format` に `V4L2_PIX_FMT_UYVY` → `VIDEO_PIXEL_FORMAT_UYVY`、`convert_video_pixel_format_to_v4l2` に `VIDEO_PIXEL_FORMAT_UYVY` → `V4L2_PIX_FMT_UYVY` を追加する。`capture_thread` の YUY2 分岐と同じパックド 4:2:2 の検証・コールバックを UYVY にも追加する。0029 の bytesperline 対応に追随する
- `src/video_pipewire.c`: `convert_spa_video_format` に `SPA_VIDEO_FORMAT_UYVY` → `VIDEO_PIXEL_FORMAT_UYVY`、`convert_video_pixel_format_to_spa` に `VIDEO_PIXEL_FORMAT_UYVY` → `SPA_VIDEO_FORMAT_UYVY` を追加する。`on_param_changed` の choice に `SPA_VIDEO_FORMAT_UYVY` を追加し、`on_process` の YUY2 分岐と同じパックド 4:2:2 の検証・コールバックを UYVY にも追加する
- `src/types.rs`: `guid_to_pixel_format` に `MFVideoFormat_UYVY` → `Some(PixelFormat::Uyvy)`、`pixel_format_to_guid` に `PixelFormat::Uyvy` → `Some(MFVideoFormat_UYVY)` を追加する
- `src/capture_mf.rs`: `process_sample` に `PixelFormat::Uyvy` の分岐を追加する。YUY2 と同じパックド 4:2:2 として扱う。既定フォーマットの候補（NV12 → YUY2 → I420）は変更しない。候補がいずれも設定できない場合はネイティブフォーマットを使用し、`get_configured_format` が `MFVideoFormat_UYVY` を返す場合は `PixelFormat::Uyvy` として扱う
- `README.md` と `skills/shiguredo-video-device/SKILL.md` の Uyvy の記述を「AVF のみ」から「全バックエンド」に更新する
- `CHANGES.md` の `## develop` に `[ADD]` エントリを追加する

## 変更対象

- `src/video_v4l2.c`: `convert_v4l2_pixel_format`、`convert_video_pixel_format_to_v4l2`、`capture_thread`
- `src/video_pipewire.c`: `convert_spa_video_format`、`convert_video_pixel_format_to_spa`、`on_param_changed`、`on_process`
- `src/types.rs`: `guid_to_pixel_format`、`pixel_format_to_guid`
- `src/capture_mf.rs`: `process_sample` の Uyvy 分岐
- `src/frame_math.rs`: `yuy2_packed_frame_bytes_win` の doc に UYVY も同じバイト数であることを追記する
- `README.md`、`skills/shiguredo-video-device/SKILL.md`: Uyvy の記述
- `CHANGES.md`: `[ADD]` エントリ

## 完了条件

- `VideoDevice::formats()` が UYVY 対応の V4L2 / PipeWire / MF デバイスで `PixelFormat::Uyvy` を報告する
- `PixelFormat::Uyvy` を指定したキャプチャで UYVY フレームが届き、`VideoFrame::pixel_format` が `Uyvy`、`uv_data` が `None`、`stride` が正の値になる
- 既存の NV12 / YUY2 / I420 / MJPEG の挙動が変わらない
- `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る
- 実機で V4L2 / PipeWire / MF の UYVY 列挙・キャプチャを確認する。CI のデバイステストで UYVY を報告するデバイスがなければ、ビルド・テストとコード照合で確認する
- `README.md` と `skills/shiguredo-video-device/SKILL.md` の記述が実装と一致し、`CHANGES.md` の `## develop` に `[ADD]` エントリがある

## スコープ外

- AVF の UYVY 対応と `PixelFormat::Uyvy` の追加は `issues/0050-bug-avf-uyvy-mislabel.md` で扱う
- `V4L2_PIX_FMT_UYVY` の bytesperline の扱いは 0029 の設計に従う
- 既定フォーマットの候補順（NV12 → YUY2 → I420）は変更しない。候補がいずれも設定できない場合はネイティブフォーマットを使用する

## 解決方法

{完了時に記入}
