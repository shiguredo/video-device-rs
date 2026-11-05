---
name: shiguredo-video-device
description: 時雨堂のクロスプラットフォームカメラキャプチャライブラリ shiguredo_video_device の機能・API リファレンス。macOS (AVFoundation) / Linux (V4L2 / PipeWire) / Windows (Media Foundation) でのデバイス列挙とフレームキャプチャ、feature によるバックエンド選択、ピクセルフォーマットとフレームのメモリレイアウト、フレームコールバックの制約、バックエンドごとの挙動差に関する質問時に使用。
---

# shiguredo_video_device

macOS / Linux / Windows のカメラ列挙とフレームキャプチャを、プラットフォーム共通の Rust API で提供するライブラリ。

## 特徴

- **クロスプラットフォーム**: macOS (AVFoundation)、Linux (V4L2 / PipeWire)、Windows (Media Foundation) を同じ API で扱う
- **feature によるバックエンド選択**: 有効化するバックエンドと既定バックエンドを feature で選ぶ
- **コールバック駆動**: キャプチャ中はバックエンドのスレッド上でフレームごとにコールバックが呼ばれる
- **ピクセルフォーマット**: NV12 / YUY2 / I420、Linux (V4L2) では MJPEG パススルー (`mjpeg` feature)
- **少ない依存**: Rust の依存クレートは Windows (`mf` feature) の `windows` クレートのみ
- **CVPixelBuffer 参照 (macOS)**: フレームに CVPixelBuffer への参照 (`PixelBuffer`) が付き、コピーせずに VideoToolbox などへ渡せる

## バージョン情報

- crate 名: `shiguredo_video_device`
- バージョン: 2026.4.0
- Rust Edition: 2024
- 最小 Rust バージョン: 1.95
- 最小 macOS バージョン: 14.0 (`AVCaptureDeviceTypeExternal` を参照するため、macOS 13 以前では動作しない)
- ライセンス: Apache-2.0
- リポジトリ: <https://github.com/shiguredo/video-device-rs>

`Error` / `PixelFormat` に `#[non_exhaustive]` は付いていない。`match` で全バリアントを列挙できる。

## 導入

```toml
[dependencies]
shiguredo_video_device = "2026.4"
```

デフォルトでは macOS は AVFoundation、Linux は V4L2、Windows は Media Foundation を既定バックエンドにする。
ビルド対象 OS 以外の feature は無視されるため、OS ごとに依存を分ける必要はない。

### feature

| feature | 対象 OS | 説明 |
| --- | --- | --- |
| `avf` | macOS | AVFoundation バックエンドを有効化する |
| `v4l2` | Linux | V4L2 バックエンドを有効化する |
| `pipewire` | Linux | PipeWire バックエンドを有効化する (`libpipewire-0.3` に動的リンクする) |
| `mf` | Windows | Media Foundation バックエンドを有効化する (`windows` クレートに依存する) |
| `mjpeg` | Linux | V4L2 で MJPEG パススルーキャプチャを有効化する (`v4l2` を含む) |
| `default-avf` | macOS | 既定バックエンドを AVFoundation にする (`avf` を含む) |
| `default-v4l2` | Linux | 既定バックエンドを V4L2 にする (`v4l2` を含む) |
| `default-pipewire` | Linux | 既定バックエンドを PipeWire にする (`pipewire` を含む) |
| `default-mf` | Windows | 既定バックエンドを Media Foundation にする (`mf` を含む) |

デフォルト feature は `default-avf` / `default-v4l2` / `default-mf`。

- 同一 OS で `default-*` を 2 つ以上有効にすると、ビルドスクリプトが panic してビルドできない
- Linux で既定を PipeWire にするには `default-features = false` が必要。このとき macOS / Windows の既定も外れるため、クロスプラットフォームのアプリでは `default-avf` / `default-mf` を指定し直す
- `default-*` が無い OS では `VideoDeviceList::enumerate()` / `VideoCapture::new()` が存在しない。`enumerate_avf()` / `new_v4l2()` などの明示 API を使う
- バックエンドが 1 つも有効でない OS では `VideoCapture` / `VideoDevice` / `VideoDeviceList` が公開されない (`Error` / `PixelFormat` などの型だけが残る)
- Linux では `v4l2` と `pipewire` を同時に有効化できる
- クレート内部の `enable_*` cfg はビルドスクリプトがこのクレートにだけ設定するもので、利用側からは参照できない。利用側は `#[cfg(target_os = "linux")]` と自身の feature で分岐する

Linux の既定を PipeWire にし、macOS / Windows は既定のまま使う場合:

```toml
[dependencies.shiguredo_video_device]
version = "2026.4"
default-features = false
features = ["default-avf", "default-mf", "default-pipewire"]
```

既定は V4L2 のまま、PipeWire と MJPEG も使えるようにする場合:

```toml
[dependencies]
shiguredo_video_device = { version = "2026.4", features = ["pipewire", "mjpeg"] }
```

### ビルド要件

| OS | 要件 |
| --- | --- |
| macOS | Xcode Command Line Tools (bindgen 用の libclang を含む) |
| Linux (V4L2) | libclang (Ubuntu では `libclang-dev`) |
| Linux (PipeWire) | libclang と `libpipewire-0.3-dev`、`pkg-config` |
| Windows | 追加の依存パッケージは不要 |

## コア API

### 公開型

| 型 | 説明 | 主要 API |
| --- | --- | --- |
| `VideoDeviceList` | デバイス列挙結果 | `enumerate()`, `enumerate_avf()`, `enumerate_v4l2()`, `enumerate_pipewire()`, `enumerate_mf()`, `as_slice()`, `len()`, `is_empty()`, `&VideoDeviceList: IntoIterator` |
| `VideoDevice` | 1 台のデバイス | `name() -> Result<String>`, `unique_id() -> Result<String>`, `formats() -> Vec<VideoFormat>`, `format_count() -> usize` |
| `VideoFormat` | デバイスが報告するフォーマット | `width: i32`, `height: i32`, `min_fps: f32`, `max_fps: f32`, `pixel_format: PixelFormat` |
| `VideoCapture` | キャプチャセッション | `new()`, `new_avf()`, `new_v4l2()`, `new_pipewire()`, `new_mf()`, `start()`, `stop()`, `config()` |
| `VideoCaptureConfig` | キャプチャ設定 (`Default` 実装あり) | `device_id: Option<String>`, `width: i32`, `height: i32`, `fps: i32`, `pixel_format: Option<PixelFormat>` |
| `VideoFrame<'a>` | コールバックに渡る借用フレーム | `data`, `uv_data`, `width`, `height`, `stride`, `stride_uv`, `pixel_format`, `timestamp_us`, `pixel_buffer`, `to_owned()` |
| `VideoFrameOwned` | 所有フレーム | `VideoFrame` と同じフィールド (`Vec<u8>` で保持), `as_frame()` |
| `PixelFormat` | ピクセルフォーマット | `Nv12`, `Yuy2`, `I420`, `Mjpeg`, `Unknown(u32)`, `name()`, `to_raw()`, `Display` |
| `PixelBuffer` | macOS の CVPixelBuffer 参照 | `as_ptr() -> *mut c_void` (Clone で CFRetain、Drop で CFRelease) |
| `Error` / `Result<T>` | エラー型と結果型 | 「エラー型」の節を参照 |

`enumerate()` / `new()` は既定バックエンド (`default-*`) を使う。`enumerate_*()` / `new_*()` は、ビルド対象 OS が一致し、対応する feature が有効なときだけ存在する。

### VideoDeviceList / VideoDevice

- `VideoDevice` は `VideoDeviceList` からの借用 (`&VideoDevice`) でしか取り出せない。リストを捨てた後も情報を使う場合は `unique_id()` などの値をコピーしておく
- `unique_id()` の値は、そのまま `VideoCaptureConfig::device_id` に渡せる (同じバックエンドに限る)
- `name()` / `unique_id()` は FFI バックエンド (macOS / Linux) で C 側が NULL や非 UTF-8 を返しうるため `Result` を返す。Windows では失敗しない
- `format_count()` は C 側が報告するエントリ数で、取得できなかったエントリを除いた `formats().len()` より大きいことがある
- `VideoFormat` の `min_fps` / `max_fps` はバックエンド依存の概算値である (「バックエンドごとの挙動差」の節を参照)

### VideoCapture のライフサイクル

- `new*()` はセッションを作るだけで、キャプチャは始まらない (macOS は権限が未決定ならここで OS のダイアログを出し、応答まで戻らない)
- `start()` は冪等で、実行中なら `Ok(())` を返す。`stop()` 後の再 `start()` も許容する
- `stop()` はブロッキングで、キャプチャスレッドとコールバックの完了を待ってから戻る。実行中でなければ何もしない
- `drop` すると、実行中なら `stop()` と同じブロッキング停止を行ってからセッションを破棄する
- `config()` は `new*()` に渡した要求値を返す。ネゴシエーション後の実際の値ではないため、解像度・フォーマット・ストライドは各フレームのフィールドを参照する

### VideoCaptureConfig

| フィールド | `Default` の値 | 説明 |
| --- | --- | --- |
| `device_id` | `None` | `VideoDevice::unique_id()` の値。`None` は既定デバイス |
| `width` | 640 | 要求する幅 |
| `height` | 480 | 要求する高さ |
| `fps` | 30 | 要求するフレームレート |
| `pixel_format` | `None` | 要求するピクセルフォーマット。`None` はバックエンドが選ぶ |

- Windows では `width` / `height` / `fps` が 0 以下だと `Error::InvalidCaptureConfig` を返す。他のバックエンドは Rust 側で検証しない
- `pixel_format: Some(PixelFormat::Unknown(_))` は全バックエンドで `Error::UnsupportedPixelFormat` を返す
- `pixel_format: Some(PixelFormat::Mjpeg)` は Linux V4L2 + `mjpeg` feature でのみ使える。それ以外は `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` を返す
- 要求値はあくまで希望であり、実際の値はバックエンドとデバイスが決める

### PixelFormat

| バリアント | 内容 | `to_raw()` (FourCC) |
| --- | --- | --- |
| `Nv12` | YUV 4:2:0 セミプラナー (Y プレーン + UV インターリーブプレーン) | `0x3231564E` (`NV12`) |
| `Yuy2` | YUV 4:2:2 パック (Y0 U Y1 V) | `0x32595559` (`YUY2`) |
| `I420` | YUV 4:2:0 プラナー (Y / U / V の 3 プレーン) | `0x30323449` (`I420`) |
| `Mjpeg` | Motion JPEG。デコードは利用者の責務 | `0x47504A4D` (`MJPG`) |
| `Unknown(u32)` | 未知の FourCC | 保持している値 |

- `name()` は `"NV12"` / `"YUY2"` / `"I420"` / `"MJPEG"` / `"Unknown"` を返す。`Display` は `Unknown` のとき `Unknown(0x????????)` 形式で値を含める
- 現状のバックエンドは `Unknown` を生成しない。未知のフォーマットは `formats()` から除外され、そのフレームはコールバックに渡らない

## フレームのメモリレイアウト

| フォーマット | `data` | `uv_data` | `stride` | `stride_uv` |
| --- | --- | --- | --- | --- |
| NV12 | Y プレーン | `Some`: UV インターリーブプレーン (`ceil(height / 2)` 行) | Y の 1 行のバイト数 | UV の 1 行のバイト数 |
| I420 | Y プレーン | `Some`: U プレーンの直後に V プレーンが続く連結領域 (各 `stride_uv * ceil(height / 2)` バイト) | Y の 1 行のバイト数 | U / V それぞれの 1 行のバイト数 |
| YUY2 | パックデータ | `None` | 1 行のバイト数 | 0 |
| MJPEG | JPEG ペイロード (長さは `data.len()`) | `None` | 0 | 0 |

- `stride` は行末のパディングを含みうる。幅ちょうどの連続バッファが必要なら行ごとに詰め直す (「コード例」の `pack_frame` を参照)
- NV12 / I420 / YUY2 の `data.len()` は、FFI バックエンド (macOS / Linux) では `stride * height`、Windows では `width * height` (YUY2 は `width * 2 * height`)
- `timestamp_us` はマイクロ秒だが起点はバックエンドごとに異なる。フレーム間の相対時間にだけ使い、壁時計の時刻としては使わない
- `pixel_buffer` は macOS では `Some`、それ以外では `None`

## フレームコールバックの制約

コールバックの型は `F: Fn(VideoFrame<'_>) + Send + 'static` である。

- `VideoFrame` のスライスはコールバックの実行中にのみ有効。コールバック後も使うなら `frame.to_owned()` で `VideoFrameOwned` にコピーする
- コールバックはバックエンドのキャプチャスレッドで実行される。重い処理をするとフレームが落ちるため、`sync_channel` + `try_send` などで別スレッドに渡す
- `FnMut` ではないため、状態の更新には `Cell` / `RefCell` / `Mutex` / アトミック型などの内部可変性を使う。`Sync` を要求しないため、コールバックが並行して呼ばれることはない
- コールバックは panic してはならない。macOS / Linux ではプロセスが abort しうる。Windows ではキャプチャスレッドが終了し、以降の `start()` が `Error::CaptureFaulted` になる。ライブラリは `catch_unwind` で吸収しない
- コールバック内から `stop()` を呼んだり `VideoCapture` を drop したりしてはならない。Linux (V4L2 / PipeWire) では use-after-free、macOS と Windows ではデッドロックになりうる。停止はチャネルやフラグで別スレッドに通知し、そのスレッドで `stop()` を呼ぶ

## スレッド安全性

| 型 | `Send` | `Sync` | 備考 |
| --- | --- | --- | --- |
| `VideoDeviceList` / `VideoDevice` | Yes | Yes | |
| `VideoCapture` (macOS / Linux) | Yes | No | |
| `VideoCapture` (Windows) | No | No | COM 初期化ガードを保持するため、作成したスレッドで使い、そのスレッドで drop する |
| `VideoFrameOwned` / `PixelBuffer` | Yes | No | `Arc<VideoFrameOwned>` はスレッド間で共有できない。複数の受信側に配るなら clone するか `Mutex` で包む |
| `Error` | Yes | Yes | `Copy` |

## コード例

### デバイス列挙

```rust
use shiguredo_video_device::VideoDeviceList;

fn main() -> shiguredo_video_device::Result<()> {
    // 既定バックエンドでデバイスを列挙する
    let devices = VideoDeviceList::enumerate()?;
    for device in &devices {
        // name() / unique_id() は FFI バックエンドで失敗しうるため Result を返す
        println!("{} ({})", device.name()?, device.unique_id()?);
        for format in device.formats() {
            println!(
                "  {}x{} {:.0}-{:.0}fps {}",
                format.width, format.height, format.min_fps, format.max_fps, format.pixel_format
            );
        }
    }
    Ok(())
}
```

### チャネル経由でフレームを受け取る (推奨パターン)

```rust
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use shiguredo_video_device::{VideoCapture, VideoCaptureConfig, VideoFrame, VideoFrameOwned};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 上限付きチャネルで背圧をかける。無制限チャネルは受信側が遅いとメモリが増え続ける
    let (tx, rx) = sync_channel::<VideoFrameOwned>(4);

    let config = VideoCaptureConfig {
        width: 1280,
        height: 720,
        fps: 30,
        ..VideoCaptureConfig::default()
    };

    let mut capture = VideoCapture::new(config, move |frame: VideoFrame<'_>| {
        // スライスはコールバック中のみ有効なので to_owned() でコピーしてから送る
        // 受信側が詰まっているときはブロックせずにフレームを捨てる
        let _ = tx.try_send(frame.to_owned());
    })?;

    capture.start()?;

    for _ in 0..30 {
        let frame = rx.recv_timeout(Duration::from_secs(5))?;
        // 実際の解像度とフォーマットはフレーム側を見る (config() は要求値)
        println!(
            "{}x{} {} ts={}us",
            frame.width, frame.height, frame.pixel_format, frame.timestamp_us
        );
    }

    // stop() はコールバックの完了を待ってから戻る。コールバック内からは呼ばない
    capture.stop();
    Ok(())
}
```

### デバイスとフォーマットを指定する

```rust
use shiguredo_video_device::{
    Error, PixelFormat, VideoCapture, VideoCaptureConfig, VideoDeviceList,
};

fn open_first_device() -> Result<VideoCapture, Box<dyn std::error::Error>> {
    let devices = VideoDeviceList::enumerate()?;
    let Some(device) = devices.as_slice().first() else {
        return Err("no video device found".into());
    };

    // デバイスが 1280x720 の NV12 を報告していればそれを要求し、無ければバックエンドに任せる
    let pixel_format = device
        .formats()
        .into_iter()
        .find(|f| f.width == 1280 && f.height == 720 && f.pixel_format == PixelFormat::Nv12)
        .map(|f| f.pixel_format);

    let config = VideoCaptureConfig {
        // unique_id() の値はそのまま device_id に渡せる
        device_id: Some(device.unique_id()?),
        width: 1280,
        height: 720,
        fps: 30,
        pixel_format,
    };

    match VideoCapture::new(config, |frame| {
        let _ = frame.data.len();
    }) {
        Ok(capture) => Ok(capture),
        Err(Error::UnsupportedPixelFormat(format)) => {
            Err(format!("pixel format {format} is not supported").into())
        }
        Err(e) => Err(e.into()),
    }
}
```

### stride を除去して連続バッファに詰める

エンコーダーなどが幅ちょうどの連続バッファを要求する場合に使う。

```rust
use shiguredo_video_device::{PixelFormat, VideoFrame};

/// stride 付きのプレーンから各行の有効バイトだけを連結する
fn pack_plane(src: &[u8], stride: usize, row_bytes: usize, rows: usize) -> Option<Vec<u8>> {
    if row_bytes > stride {
        return None;
    }
    // 寸法はデバイス由来で信用できないため、事前確保せずに実データ分だけ伸ばす
    let mut out = Vec::new();
    for row in 0..rows {
        let start = row.checked_mul(stride)?;
        out.extend_from_slice(src.get(start..start.checked_add(row_bytes)?)?);
    }
    Some(out)
}

/// フレームを stride なしの連続バッファに詰める。寸法が不整合なら None を返す
fn pack_frame(frame: &VideoFrame<'_>) -> Option<Vec<u8>> {
    let w = usize::try_from(frame.width).ok()?;
    let h = usize::try_from(frame.height).ok()?;
    let stride = usize::try_from(frame.stride).ok()?;
    let stride_uv = usize::try_from(frame.stride_uv).ok()?;
    let chroma_w = w.div_ceil(2);
    let chroma_h = h.div_ceil(2);
    match frame.pixel_format {
        PixelFormat::Nv12 => {
            let mut out = pack_plane(frame.data, stride, w, h)?;
            // UV は U と V が交互に並ぶため、1 行の有効バイト数は chroma_w * 2
            out.extend(pack_plane(
                frame.uv_data?,
                stride_uv,
                chroma_w * 2,
                chroma_h,
            )?);
            Some(out)
        }
        PixelFormat::I420 => {
            let uv = frame.uv_data?;
            // uv_data は U プレーンの直後に V プレーンが続く
            let plane_size = stride_uv.checked_mul(chroma_h)?;
            let mut out = pack_plane(frame.data, stride, w, h)?;
            out.extend(pack_plane(
                uv.get(..plane_size)?,
                stride_uv,
                chroma_w,
                chroma_h,
            )?);
            out.extend(pack_plane(
                uv.get(plane_size..)?,
                stride_uv,
                chroma_w,
                chroma_h,
            )?);
            Some(out)
        }
        // YUY2 は 2 ピクセルで 4 バイト
        PixelFormat::Yuy2 => pack_plane(frame.data, stride, chroma_w * 4, h),
        // MJPEG は stride を持たない JPEG ペイロード
        PixelFormat::Mjpeg => Some(frame.data.to_vec()),
        PixelFormat::Unknown(_) => None,
    }
}
```

### 既定以外のバックエンドを明示する (Linux PipeWire)

```rust
// 依存の features で pipewire を有効にしている前提
// 他 OS で unused import にならないよう、use にも cfg を付ける
#[cfg(target_os = "linux")]
use shiguredo_video_device::{VideoCapture, VideoCaptureConfig, VideoDeviceList};

#[cfg(target_os = "linux")]
fn start_pipewire_capture() -> shiguredo_video_device::Result<VideoCapture> {
    let devices = VideoDeviceList::enumerate_pipewire()?;
    // PipeWire の unique_id() は node name で、そのまま device_id に渡せる
    let device_id = devices
        .as_slice()
        .first()
        .map(|d| d.unique_id())
        .transpose()?;
    let config = VideoCaptureConfig {
        device_id,
        ..VideoCaptureConfig::default()
    };
    let mut capture = VideoCapture::new_pipewire(config, |frame| {
        // PipeWire は要求と異なるフォーマットに交渉しうるため、毎フレーム pixel_format を確認する
        let _ = frame.pixel_format;
    })?;
    // PipeWire の start() はストリーミング状態になるまでブロックする
    capture.start()?;
    Ok(capture)
}
```

### MJPEG パススルー (Linux V4L2)

```rust
// 依存の features で mjpeg を有効にしている前提
#[cfg(target_os = "linux")]
use shiguredo_video_device::{PixelFormat, VideoCapture, VideoCaptureConfig};

#[cfg(target_os = "linux")]
fn start_mjpeg_capture() -> shiguredo_video_device::Result<VideoCapture> {
    let config = VideoCaptureConfig {
        pixel_format: Some(PixelFormat::Mjpeg),
        ..VideoCaptureConfig::default()
    };
    // MJPEG は V4L2 だけが対応する。既定が PipeWire の構成でも動くよう new_v4l2() を明示する
    let mut capture = VideoCapture::new_v4l2(config, |frame| {
        // data は JPEG ペイロードそのもの。uv_data は None、stride / stride_uv は 0
        let jpeg: &[u8] = frame.data;
        let _ = jpeg;
    })?;
    capture.start()?;
    Ok(capture)
}
```

### CVPixelBuffer を取り出す (macOS)

```rust
#[cfg(target_os = "macos")]
use shiguredo_video_device::VideoFrame;

#[cfg(target_os = "macos")]
fn handle_frame(frame: &VideoFrame<'_>) {
    if let Some(pixel_buffer) = &frame.pixel_buffer {
        // CVPixelBufferRef。VideoToolbox などにコピーせず渡せる
        // 寿命は PixelBuffer に従うため、保持するなら pixel_buffer を clone (CFRetain) する
        let _cv_pixel_buffer: *mut std::ffi::c_void = pixel_buffer.as_ptr();
    }
}
```

`to_owned()` も `PixelBuffer` を clone (CFRetain) する。`VideoFrameOwned` を溜め込むと CVPixelBuffer が AVFoundation に返らずフレームが落ちうるため、不要なら `owned.pixel_buffer = None` で早めに解放する。

## バックエンドごとの挙動差

| 項目 | macOS (AVF) | Linux (V4L2) | Linux (PipeWire) | Windows (MF) |
| --- | --- | --- | --- | --- |
| `unique_id()` の値 | `AVCaptureDevice.uniqueID` | `/dev/videoN` | node name | シンボリックリンク |
| `device_id: None` の既定 | システムの既定デバイス | `/dev/video0` 固定 | 自動接続先 | 列挙順の先頭 |
| 存在しない `device_id` | 既定デバイスへ無言でフォールバック | `SessionCreateFailed` | セッションマネージャーの挙動に依存 | `DeviceNotFound` |
| `pixel_format: None` | NV12 のみ要求 | NV12、YUY2 の順に試す | NV12 を優先し YUY2 / I420 も候補 | NV12、YUY2、I420 の順に試し、全滅時はネイティブ形式 |
| 指定フォーマット非対応時 | `SessionCreateFailed` | `SessionCreateFailed` | YUY2 / I420 に交渉されうる | `UnsupportedPixelFormat` |
| 解像度 | 出力を要求解像度に指定 (スケーリング) | ドライバが近い値に調整 | 1x1 から 3840x2160 の範囲で交渉 | 完全一致を要求 (ネイティブ形式へのフォールバック時は除く) |
| fps | 対応範囲外なら最も近い範囲 | 設定失敗は無視 | 1 から 60 の範囲で交渉 | 完全一致を要求 |
| `start()` | セッション開始まで同期 | 即座に戻る | ストリーミング状態までブロック | 即座に戻る |
| コールバックのスレッド | 専用のシリアル dispatch queue | 専用 pthread | PipeWire のデータループ | 専用 Rust スレッド |
| `timestamp_us` の起点 | サンプルの PTS | V4L2 バッファのタイムスタンプ | バッファの PTS (無ければ `CLOCK_MONOTONIC`) | `ReadSample` のタイムスタンプ |
| `VideoFormat` の fps | 対応範囲の最小 / 最大 | 列挙値 (取得失敗時は 1.0 / 30.0) | 常に 1.0 / 30.0 (実測値ではない) | 公称値 (min = max) |

- macOS の `formats()` はデバイスのネイティブフォーマットである。キャプチャ時は AVFoundation が出力形式に変換するため、`formats()` に無いピクセルフォーマットも要求できる場合がある
- macOS の NV12 はビデオレンジ (`420v`) を要求している
- V4L2 の列挙は `/dev/video0` から `/dev/video63` のうち、キャプチャとストリーミングに対応するノードだけを返す。stepwise / continuous のフレームサイズは最大サイズの 1 件だけを報告する

## エラー型

`Error` は `Debug` / `Clone` / `Copy` / `Display` (英語メッセージ) / `std::error::Error` を実装する。`Result<T>` は `std::result::Result<T, Error>` の別名。

| バリアント | 意味 | 主な発生箇所 |
| --- | --- | --- |
| `DeviceNotFound` | 指定デバイスが見つからない | Windows: `device_id` に一致するデバイスが無い、デバイスが 1 台も無い |
| `DeviceAccessDenied` | デバイスへのアクセスが拒否された | 列挙の失敗 (FFI バックエンドでは失敗理由を問わずこれになる) |
| `SessionCreateFailed` | セッション作成の失敗 | FFI: `new*()` の失敗全般 (権限拒否・デバイス不在・フォーマット非対応を含む)、Windows: MF の初期化やメディアタイプの取得失敗 |
| `SessionStartFailed` | セッション開始の失敗 | `start()` |
| `UnsupportedPixelFormat(PixelFormat)` | 非対応のピクセルフォーマット | `Unknown(_)` の指定、MJPEG 非対応環境での `Mjpeg` の指定、Windows での指定フォーマットの設定失敗 |
| `NullPointer(&'static str)` | C 側が NULL を返した | FFI: `name()` / `unique_id()`、NUL バイトを含む `device_id` |
| `InvalidCaptureConfig(&'static str)` | 設定がプラットフォーム要件を満たさない | Windows: `width` / `height` / `fps` が 0 以下 |
| `ComInitFailed` | COM の初期化に失敗した | Windows: STA で初期化済みのスレッドから呼んだ場合など |
| `InvalidUtf8(&'static str)` | C 側の文字列が UTF-8 でない | FFI: `name()` / `unique_id()` |
| `CaptureFaulted` | キャプチャスレッドの panic 後で再開できない | Windows: コールバックの panic 後の `start()`。新しい `VideoCapture` を作り直す |

## プラットフォーム固有の注意

### macOS

- `new()` / `new_avf()` はカメラ権限が未決定ならその場で OS のダイアログを出し、応答まで戻らない。拒否・制限されている場合は `SessionCreateFailed` になる
- アプリバンドルとして配布する場合は、情報プロパティリストに `NSCameraUsageDescription` キーが必要
- 列挙対象は内蔵カメラ (`AVCaptureDeviceTypeBuiltInWideAngleCamera`) と外部カメラ (`AVCaptureDeviceTypeExternal`) だけで、Continuity Camera や Desk View Camera は列挙されない

### Linux

- `/dev/video*` へのアクセスには実行ユーザーが `video` グループに所属している必要がある (`sudo usermod -aG video <ユーザー名>` の後に再ログイン)
- PipeWire のカメラ認識は環境によって不安定な場合がある。`wpctl status` の Video セクションにデバイスが表示されることを確認する。安定性を重視するなら V4L2 を使う

### Windows

- Media Foundation を使うため、呼び出し元スレッドの COM を MTA (`COINIT_MULTITHREADED`) で初期化する。STA で初期化済みのスレッド (WinForms / WPF の UI スレッドなど) から `enumerate()` / `new()` を呼ぶと `ComInitFailed` になるため、別スレッドで呼ぶ
- `VideoCapture` は `!Send` なので、作成したスレッドで使い、そのスレッドで drop する
- キャプチャスレッドが panic すると、`stop()` 時に stderr へログを出し、以降の `start()` は `CaptureFaulted` になる

## 既知の制限事項

2026.4.0 時点の制限。

- 共通
  - コールバック内での `stop()` 呼び出しと `VideoCapture` の drop は safe Rust で書けてしまうが、use-after-free やデッドロックを起こす
  - FFI バックエンドはエラーの種別が粗く、権限拒否・デバイス不在・フォーマット非対応がいずれも `SessionCreateFailed` になる
- macOS
  - 存在しない `device_id` を渡すと、既定デバイスで無言でキャプチャを始める
  - `pixel_format: None` は NV12 しか要求しないため、NV12 を出力できないカメラではキャプチャできない
  - UYVY (`2vuy`) のフォーマットを YUY2 として `formats()` に報告する
- Linux (V4L2)
  - `stride` にドライバの `bytesperline` ではなく幅から算出した値を使うため、行パディングのあるドライバでは映像が崩れる
- Linux (PipeWire)
  - `VideoFormat` の fps は常に 1.0 / 30.0 の固定値
  - `stop()` から `start()` を繰り返すと、内部の接続とストリームがリークする
  - 3840x2160 / 60fps を超える要求では、要求と異なる解像度・フレームレートがエラー無しで返る
  - 複数プレーンのバッファや想定外のフォーマットに交渉されると、`start()` は成功するがフレームが届かないことがある
- ビルド
  - Linux で `--all-features` を指定すると `default-v4l2` と `default-pipewire` が同時に有効になり、ビルドスクリプトが panic する
