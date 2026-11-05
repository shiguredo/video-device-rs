# Linux (V4L2 / PipeWire) と Windows (MF) で UYVY を列挙・キャプチャできるようにする

- Priority: Low
- Created: 2026-11-05
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/add-uyvy-linux-windows
- Polished: 2026-11-06

## 目的

`issues/0050-bug-avf-uyvy-mislabel.md` で `PixelFormat::Uyvy` を追加するが、AVF のみ対応のままでは Linux (V4L2 / PipeWire) と Windows (MF) の利用者が UYVY を扱えない。各バックエンドで UYVY を列挙・キャプチャできるようにする。

## 現状

`src/video_v4l2.c` の `convert_v4l2_pixel_format` と `convert_video_pixel_format_to_v4l2` は NV12 / YUY2 / I420 / MJPEG のみを変換し、`V4L2_PIX_FMT_UYVY` を扱わない。`capture_thread` も UYVY の分岐を持たない。

`src/video_pipewire.c` の `convert_spa_video_format` と `convert_video_pixel_format_to_spa` は NV12 / YUY2 / I420 のみを変換し、`SPA_VIDEO_FORMAT_UYVY` を扱わない。`on_process` の分岐にも UYVY がない。`video_pipewire_session_start` の `SPA_POD_CHOICE_ENUM_Id` にも UYVY がない（`on_param_changed` は choice を持たず、`convert_spa_video_format` で交渉結果を変換するだけである）。

`src/capture_mf.rs` は `PixelFormat::Yuy2` の分岐を持つが、`PixelFormat::Uyvy` を実際に処理する分岐を持たない（0050 完了時点では到達しないプレースホルダ分岐だけがある）。`src/types.rs` の `guid_to_pixel_format` / `pixel_format_to_guid` も `MFVideoFormat_UYVY` を扱わない。

`src/capture_ffi.rs` の `frame_callback` には 0050 で `PixelFormat::Uyvy` の分岐が追加されるが、C 側が `VIDEO_PIXEL_FORMAT_UYVY` を返さないため到達しない。

## 前提

- `issues/0050-bug-avf-uyvy-mislabel.md` で `PixelFormat::Uyvy` と `VIDEO_PIXEL_FORMAT_UYVY` が追加されていること。0050 が未完了の場合は `PixelFormat::Uyvy` が存在しないため、0050 の完了後に着手する
- `issues/0029-bug-v4l2-bytesperline-stride.md` で V4L2 の `bytesperline` 対応が実装されていること。0029 が未完了の場合は `session->bytesperline` が存在しないため、0029 の完了後に着手する
- PipeWire のパックド 4:2:2 の長さ契約は `issues/0046-bug-pipewire-frame-buffer-contract.md` の修正に従う。0046 が未完了の場合は UYVY も YUY2 と同じ検証長の不整合を引き継ぐため、0046 の完了後に着手する
- `issues/0070-refactor-types-mf-separation.md` が先に実施された場合は、`src/types.rs` の `guid_to_pixel_format` / `pixel_format_to_guid` は `src/mf.rs` に移っているため、これらの変更対象を `src/mf.rs` に読み替える（`PixelFormat::Uyvy` の rustdoc は `src/types.rs` のまま）

## 設計方針

- `src/video_v4l2.c`: `convert_v4l2_pixel_format` に `V4L2_PIX_FMT_UYVY` → `VIDEO_PIXEL_FORMAT_UYVY`、`convert_video_pixel_format_to_v4l2` に `VIDEO_PIXEL_FORMAT_UYVY` → `V4L2_PIX_FMT_UYVY` を追加する。`capture_thread` の UYVY 分岐は YUY2 と同じパックド 4:2:2 とし、0029 で追加される `session->bytesperline` を stride に使う。`bytesperline == 0` の場合は YUY2 と同じく `session->width * 2` にフォールバックする
- `src/video_pipewire.c`: `convert_spa_video_format` に `SPA_VIDEO_FORMAT_UYVY` → `VIDEO_PIXEL_FORMAT_UYVY`、`convert_video_pixel_format_to_spa` に `VIDEO_PIXEL_FORMAT_UYVY` → `SPA_VIDEO_FORMAT_UYVY` を追加する。`video_pipewire_session_start` の `SPA_POD_CHOICE_ENUM_Id` に `SPA_VIDEO_FORMAT_UYVY` を既存の候補（YUY2 / I420）の後ろに追加する（`requested_format` が `SPA_VIDEO_FORMAT_UYVY` のときは既に含まれるため追加せず、それ以外のときは値の個数も 1 増やす）。`on_process` の UYVY 分岐は YUY2 と同じパックド 4:2:2 とし、0046 で修正される YUY2 の検証長（`stride * height`）にそろえる。`on_param_changed` は `convert_spa_video_format` で交渉結果を変換するだけで、choice は持たない
- `src/types.rs`: `guid_to_pixel_format` に `MFVideoFormat_UYVY` → `Some(PixelFormat::Uyvy)` を追加する。`pixel_format_to_guid` の `PixelFormat::Uyvy` アームは 0050 で `None` になっているため、`Some(MFVideoFormat_UYVY)` に変更する。`PixelFormat::Uyvy` の rustdoc を「AVF のみ」から「AVF / V4L2 / PipeWire / MF で列挙・キャプチャ可能」に更新し、デバイスが UYVY 非対応の場合のエラーはバックエンド依存であることを書く
- `src/capture_mf.rs`: `process_sample` の `PixelFormat::Uyvy` 分岐（0050 で追加されるプレースホルダ）を、YUY2 と同じパックド 4:2:2 の処理に置き換える。`yuy2_packed_frame_bytes_win` でバイト数を計算し、`stride` は `width * 2`、`uv_data` は `None`、`stride_uv` は 0 とする。既定フォーマットの候補（NV12 → YUY2 → I420）は変更しない。候補がいずれも設定できない場合はネイティブフォーマットを使用し、`get_configured_format` が `MFVideoFormat_UYVY` を返す場合は `PixelFormat::Uyvy` として扱う
- `src/video.h`: `FrameCallback` のコメントに UYVY のレイアウト（`data` はパックドデータ、`uv_data` は NULL、`stride` はバイト/行）を追記する
- `README.md` と `skills/shiguredo-video-device/SKILL.md` の Uyvy の記述を「AVF のみ」から「全バックエンド」に更新する
- `CHANGES.md` の `## develop` に `[ADD]` エントリを追加する

## 変更対象

- `src/video_v4l2.c`: `convert_v4l2_pixel_format`、`convert_video_pixel_format_to_v4l2`、`capture_thread`（UYVY の bytesperline と `bytesperline == 0` のフォールバックを含む）
- `src/video_pipewire.c`: `convert_spa_video_format`、`convert_video_pixel_format_to_spa`、`video_pipewire_session_start`（`SPA_POD_CHOICE_ENUM_Id`）、`on_process`
- `src/types.rs`: `PixelFormat::Uyvy` の rustdoc、`guid_to_pixel_format`、`pixel_format_to_guid`（`Uyvy` アームは `None` から `Some` への変更）
- `src/capture_mf.rs`: `process_sample` の Uyvy 分岐（0050 のプレースホルダを置き換え）
- `src/video.h`: `FrameCallback` のコメント（UYVY のレイアウト）
- `src/frame_math.rs`: `yuy2_packed_frame_bytes_win` の doc に UYVY も同じバイト数であることを追記する。モジュール doc のフォーマット一覧に UYVY を追加する
- `tests/test_pipewire.rs`: `pipewire_enumerate_returns_formats` の `pixel_format` の許容リストに `UYVY` を追加する
- `README.md`、`skills/shiguredo-video-device/SKILL.md`: Uyvy の記述
- `CHANGES.md`: `[ADD]` エントリ

## 完了条件

- `VideoDevice::formats()` が UYVY 対応の V4L2 / PipeWire / MF デバイスで `PixelFormat::Uyvy` を報告する（PipeWire は 0025 の choice 制限に従い、単一値として取得できる場合に限る）
- `PixelFormat::Uyvy` を指定したキャプチャで UYVY フレームが届き、`VideoFrame::pixel_format` が `Uyvy`、`uv_data` が `None`、`stride` が正の値になる
- V4L2 の UYVY キャプチャで `bytesperline` が stride に使われ、`bytesperline == 0` のときは `width * 2` にフォールバックする
- MF の `pixel_format_to_guid(PixelFormat::Uyvy)` が `Some(MFVideoFormat_UYVY)` を返し、`guid_to_pixel_format(MFVideoFormat_UYVY)` が `Some(PixelFormat::Uyvy)` を返す
- 既存の NV12 / YUY2 / I420 / MJPEG の挙動が変わらない
- `tests/test_pipewire.rs` の `pipewire_enumerate_returns_formats` が PipeWire の `formats()` に `PixelFormat::Uyvy` が含まれても失敗しない
- `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る
- Linux で `cargo check --workspace --features v4l2,pipewire`、`cargo test --workspace --features v4l2,pipewire`、`cargo clippy --workspace --all-targets --features v4l2,pipewire -- -D warnings` が通る（`src/video_pipewire.c` と `tests/test_pipewire.rs` を対象に含めるため）
- 実機で V4L2 / PipeWire / MF の UYVY 列挙・キャプチャを確認する。CI のデバイステストで UYVY を報告するデバイスがなければ、ビルド・テストとコード照合で確認する
- `README.md` と `skills/shiguredo-video-device/SKILL.md` の記述が実装と一致し、`src/types.rs` の `PixelFormat::Uyvy` の rustdoc と `src/video.h` の `FrameCallback` のコメントが実装と一致し、`CHANGES.md` の `## develop` に `[ADD]` エントリがある

## スコープ外

- AVF の UYVY 対応と `PixelFormat::Uyvy` の追加は `issues/0050-bug-avf-uyvy-mislabel.md` で扱う
- `V4L2_PIX_FMT_UYVY` の bytesperline の扱いは 0029 の設計に従う（UYVY も YUY2 と同じ `bytesperline` / `bytesperline == 0` フォールバック）
- MF の既定フォーマットの候補順（NV12 → YUY2 → I420）は変更しない。候補がいずれも設定できない場合はネイティブフォーマットを使用する
- PipeWire のフレームバッファ契約（複数プレーン、stride 0、VideoCrop、整数オーバーフロー）は `issues/0046-bug-pipewire-frame-buffer-contract.md` で扱う
- PipeWire の choice 形式の展開は `issues/closed/0025-bug-pipewire-enumerate-formats.md` の制限に従う（0073 では choice 展開を変更しない）
- `issues/0070-refactor-types-mf-separation.md` の MF ヘルパ分離は 0070 で扱う

## 解決方法

{完了時に記入}
