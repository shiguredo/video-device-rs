# AVF が UYVY（'2vuy'）を YUY2 として扱う

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-uyvy-mislabel
- Polished: 2026-11-05

## 目的

`src/video_avf.m` の `convert_pixel_format` と `captureOutput` が、`kCVPixelFormatType_422YpCbCr8`（FourCC '2vuy' = UYVY、Cb Y'0 Cr Y'1 の並び）を `PixelFormat::Yuy2` として扱っている。`kCVPixelFormatType_422YpCbCr8_yuvs`（'yuvs' = YUY2、Y'0 Cb Y'1 Cr の並び）とはバイト順が異なる別フォーマットである。`PixelFormat` には UYVY に対応する variant がないため、列挙とフレームの両方で誤ったラベルが付く。

結果として次の問題がある。

- `VideoDevice::formats()` が '2vuy' のみをネイティブ対応するカメラを「YUY2 対応」と報告する。利用者が `PixelFormat::Yuy2` を要求すると 'yuvs' が要求され、'yuvs' を出力できないカメラでは `Error::SessionCreateFailed` になる
- 何らかの経路で '2vuy' のフレームが届いた場合、Cb と Cr が入れ替わった色で利用者へ渡る

## 優先度根拠

- Medium。列挙結果の誤りと色データの誤認につながる
- 既定の `pixel_format: None` は '420v'（NV12）を要求し、'yuvs' は `PixelFormat::Yuy2` を明示指定したときだけ要求する。そのため通常のキャプチャで '2vuy' のフレームが届くことは少ないが、'2vuy' のみをネイティブ対応する外部カメラでは列挙とキャプチャの不一致が起きる
- `/review-code` の指摘として確認済み

## 現状

`src/video_avf.m` の `captureOutput` は次のとおり。

```objc
} else if (pixelFormat == kCVPixelFormatType_422YpCbCr8_yuvs ||
           pixelFormat == kCVPixelFormatType_422YpCbCr8) {
    // YUY2 形式
    const uint8_t* data = CVPixelBufferGetBaseAddress(imageBuffer);
    size_t stride = CVPixelBufferGetBytesPerRow(imageBuffer);

    CFRetain(imageBuffer);
    self.callback(self.userData, data, NULL, (int)width, (int)height,
                  (int)stride, 0, VIDEO_PIXEL_FORMAT_YUY2, timestamp_us,
                  (void*)imageBuffer);
```

`convert_pixel_format` も同じ 2 つを `VIDEO_PIXEL_FORMAT_YUY2` に変換している。

```objc
case kCVPixelFormatType_422YpCbCr8_yuvs:
case kCVPixelFormatType_422YpCbCr8:
    return VIDEO_PIXEL_FORMAT_YUY2;
```

`convert_video_pixel_format_to_cv` は `VIDEO_PIXEL_FORMAT_YUY2` を `kCVPixelFormatType_422YpCbCr8_yuvs` に変換するが、UYVY に対応するタグはない。

```objc
case VIDEO_PIXEL_FORMAT_YUY2:
    return kCVPixelFormatType_422YpCbCr8_yuvs;
```

`src/types.rs` の `PixelFormat` は `Nv12` / `Yuy2` / `I420` / `Mjpeg` / `Unknown(u32)` のみで、UYVY を表す variant がない。`from_raw` / `to_raw` も `VIDEO_PIXEL_FORMAT_*` の 4 つだけを変換する。

macOS SDK の `CVPixelBuffer.h` と Apple Developer Documentation では、次のように定義されている。

- `kCVPixelFormatType_422YpCbCr8`: "Component Y’CbCr 8-bit 4:2:2, ordered Cb Y’0 Cr Y’1."（'2vuy' = UYVY）
- `kCVPixelFormatType_422YpCbCr8_yuvs`: "Component Y’CbCr 8-bit 4:2:2, ordered Y’0 Cb Y’1 Cr."（'yuvs' = YUY2）

参照:

- <https://developer.apple.com/documentation/corevideo/kcvpixelformattype_422ypcbcr8>
- <https://developer.apple.com/documentation/corevideo/kcvpixelformattype_422ypcbcr8_yuvs>
- <https://developer.apple.com/documentation/avfoundation/avcapturevideodataoutput/availablevideocvpixelformattypes>

## 設計方針

`PixelFormat` に `Uyvy` variant を追加し、'2vuy' を UYVY として扱う。'2vuy' を列挙・キャプチャの対象外にする案は、'2vuy' のみをネイティブ対応するカメラで `formats()` が空になり、`device_list` / `device_info` の `format_count > 0` も満たせないため採用しない。

- `src/video.h` に `VIDEO_PIXEL_FORMAT_UYVY` を追加する。値は既存の `VIDEO_PIXEL_FORMAT_YUY2` が 'YUY2' であるのと同様に 'UYVY'（`0x59565955`）とする。macOS の '2vuy'（`0x32767579`）は AVF の変換関数で扱う
- `src/types.rs` の `PixelFormat` に `Uyvy` を追加し、`from_raw` / `to_raw` / `name` / `Display` を対応させる。`name()` は `"UYVY"` を返す。`Uyvy` の rustdoc には AVF でのみ列挙・キャプチャ可能であることと、他バックエンドで要求した場合のエラーを明記する。`PixelFormat` は `#[non_exhaustive]` ではないため、variant の追加は後方互換のない変更として `CHANGES.md` の `[CHANGE]` に記載する
- `src/video_avf.m` の `convert_pixel_format` は `kCVPixelFormatType_422YpCbCr8` を `VIDEO_PIXEL_FORMAT_UYVY` に、`kCVPixelFormatType_422YpCbCr8_yuvs` を `VIDEO_PIXEL_FORMAT_YUY2` に変換する
- `src/video_avf.m` の `captureOutput` は '2vuy' を `VIDEO_PIXEL_FORMAT_UYVY` としてコールバックする。フレームの契約は YUY2 と同じパックド 4:2:2 で、`data` はパックドデータ、`uv_data` は NULL、`stride` は `CVPixelBufferGetBytesPerRow`、`stride_uv` は 0 とする
- `src/video_avf.m` の `convert_video_pixel_format_to_cv` に `VIDEO_PIXEL_FORMAT_UYVY` → `kCVPixelFormatType_422YpCbCr8` を追加する
- `src/capture_ffi.rs` の `frame_callback` に `PixelFormat::Uyvy` の分岐を追加する。バイト数は YUY2 と同じ `frame_math::yuy2_packed_frame_bytes` で計算できる
- `src/capture_mf.rs` の `process_sample` に `PixelFormat::Uyvy` の分岐を追加する。`PixelFormat::Mjpeg` と同様に、`pixel_format_to_guid` が `None` を返す場合は到達しない分岐として扱う。`src/types.rs` の `pixel_format_to_guid` には `PixelFormat::Uyvy` → `None` のアームを追加する（MF の UYVY 対応は 0073 で扱う）
- `examples/camera_preview.rs` の `enqueue_video_frame` に `PixelFormat::Uyvy` の分岐を追加する。`raw_player` は UYVY を扱わないため、`PixelFormat::Mjpeg` と同様に警告してスキップする
- `pbt/tests/prop_types.rs` と `pbt/tests/prop_error.rs` の `PixelFormat` のサンプルに `Uyvy` を追加する。`prop_to_raw_matches_expected_fourcc` の期待値は `0x59565955`、`prop_name_and_display_are_consistent` の期待値は `"UYVY"` とする
- 0049 の AVF フォーマットフォールバック実装を前提とし、その候補（NV12 → YUY2 → I420）の YUY2 の次、I420 の前に Uyvy を追加する。0049 が未実装の状態で 0050 を実装する場合は、0049 の完了を待ってから追加する
- `README.md` の「対応ピクセルフォーマット」と「特徴」、`skills/shiguredo-video-device/SKILL.md` の `PixelFormat` 表・フレームのメモリレイアウト・バックエンド差・既知の制限事項を更新する。SKILL.md の「UYVY ('2vuy') のフォーマットを YUY2 として formats() に報告する」は削除する
- `CHANGES.md` の `## develop` に `[CHANGE]` エントリを追加する

## 変更対象

- `src/video.h`: `VIDEO_PIXEL_FORMAT_UYVY`
- `src/types.rs`: `PixelFormat::Uyvy`（rustdoc を含む）、`from_raw` / `to_raw` / `name` / `Display`、`pixel_format_to_guid` の Uyvy アーム、`VideoFrame` / `VideoFrameOwned` の `uv_data` の doc
- `src/video_avf.m`: `convert_pixel_format`、`captureOutput`、`convert_video_pixel_format_to_cv`
- `src/capture_ffi.rs`: `frame_callback` の Uyvy 分岐
- `src/capture_mf.rs`: `process_sample` の Uyvy 分岐
- `src/frame_math.rs`: `yuy2_packed_frame_bytes` の doc に UYVY も同じバイト数であることを追記する
- `examples/camera_preview.rs`: `enqueue_video_frame` の Uyvy 分岐
- `pbt/tests/prop_types.rs`、`pbt/tests/prop_error.rs`: `PixelFormat` のサンプル
- `README.md`、`skills/shiguredo-video-device/SKILL.md`: Uyvy の追記と既知の制限事項の更新
- `CHANGES.md`: `[CHANGE]` エントリ

## 完了条件

- `PixelFormat::Uyvy` が追加され、`to_raw()` が `0x59565955`、`from_raw(0x59565955)` が `Uyvy`、`name()` が `"UYVY"` を返す
- `VideoDevice::formats()` が '2vuy' を `PixelFormat::Yuy2` として報告せず、`PixelFormat::Uyvy` として報告する
- `PixelFormat::Uyvy` を指定した AVF キャプチャで '2vuy' のフレームが届き、`VideoFrame::pixel_format` が `Uyvy`、`uv_data` が `None`、`stride` が正の値になる
- 'yuvs' の既存挙動が変わらない。`VideoDevice::formats()` は 'yuvs' を `PixelFormat::Yuy2` として報告し、`PixelFormat::Yuy2` を指定したキャプチャは従来どおり 'yuvs' を要求する
- 0049 の実装後、`pixel_format: None` で '2vuy' のみをネイティブ対応するカメラから Uyvy フレームが届く
- `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る
- macOS 実機で、'2vuy' を報告するカメラを使って上記の列挙とキャプチャを確認する。CI の macOS は TCC のためキャプチャテストを実行しないので、手元の実機で確認する
- `README.md` と `skills/shiguredo-video-device/SKILL.md` の記述が実装と一致し、`CHANGES.md` の `## develop` に `[CHANGE]` エントリがある

## スコープ外

- Linux (V4L2 / PipeWire) と Windows (MF) の UYVY 列挙・キャプチャ対応は `issues/0073-add-uyvy-linux-windows.md` で扱う。本 issue では `PixelFormat::Uyvy` の追加と、コンパイルを通すための `capture_ffi` / `capture_mf` / `camera_preview` / PBT の分岐だけを追加する
- 0049 の既存候補の順（NV12 → YUY2 → I420）は維持し、Uyvy を YUY2 の次、I420 の前に挿入する
- `Error` の種別細分化（FFI バックエンドで `UnsupportedPixelFormat` を返す等）は 0052 で扱う
- 本 issue の `PixelFormat::Uyvy` は AVF でのみ列挙・キャプチャ可能とする。他バックエンドで `PixelFormat::Uyvy` を要求した場合は、既存の未対応フォーマット扱いに従う（FFI は `Error::SessionCreateFailed`、MF は `Error::UnsupportedPixelFormat`）

## 解決方法

{完了時に記入}
