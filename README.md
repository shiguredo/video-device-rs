# video-device-rs

[![crates.io](https://img.shields.io/crates/v/shiguredo_video_device.svg)](https://crates.io/crates/shiguredo_video_device)
[![docs.rs](https://docs.rs/shiguredo_video_device/badge.svg)](https://docs.rs/shiguredo_video_device)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![GitHub Actions](https://github.com/shiguredo/video-device-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/shiguredo/video-device-rs/actions/workflows/ci.yml)
[![Discord](https://img.shields.io/badge/Discord-%235865F2.svg?logo=discord&logoColor=white)](https://discord.gg/shiguredo)

## About Shiguredo's open source software

We will not respond to PRs or issues that have not been discussed on Discord. Also, Discord is only available in Japanese.

Please read <https://github.com/shiguredo/oss> before use.

## 時雨堂のオープンソースソフトウェアについて

利用前に <https://github.com/shiguredo/oss> をお読みください。

## 概要

macOS / Linux / Windows に対応したビデオデバイスライブラリです。
カメラの列挙とフレームキャプチャを、プラットフォーム共通の Rust API で提供します。

## 特徴

- クロスプラットフォームのカメラ列挙 / キャプチャ API
- バックエンドを feature flag で選択可能
- コールバックによるフレーム受信
- ピクセルフォーマット選択 (`NV12` / `YUY2` / `I420` / `MJPEG`)
- Linux (V4L2) での MJPEG パススルーキャプチャ (`mjpeg` feature)
- 実行時に依存するクレートは Windows (`mf` feature) の `windows` クレートのみ

## 対応プラットフォーム

- macOS: AVFoundation (`avf`)
- Linux: V4L2 (`v4l2`, デフォルト) / PipeWire (`pipewire`)
- Windows: Media Foundation (`mf`)

Linux のパーミッションや PipeWire の注意点は [docs/LINUX.md](docs/LINUX.md) を参照してください。

## 動作要件

- Rust 1.95 以上 (`rust-version = "1.95"`)
- macOS 14.0 以上 (`AVCaptureDeviceTypeExternal` を利用するため)

## feature

バックエンドは feature flag で有効化します。
`VideoCapture::new()` と `VideoDeviceList::enumerate()` が使う既定バックエンドは、各プラットフォームの `default-*` feature で選びます。
同一プラットフォームで `default-*` を複数指定するとビルドエラーになります。

デフォルトの feature は `default-avf` / `default-v4l2` / `default-mf` です。
ビルド対象 OS 以外の feature は無視されます。

| プラットフォーム | バックエンド有効化 | 既定バックエンド |
| --- | --- | --- |
| macOS | `avf` | `default-avf` (`avf` を含む) |
| Linux | `v4l2` / `pipewire` | `default-v4l2` (`v4l2` を含む) または `default-pipewire` (`pipewire` を含む) |
| Windows | `mf` | `default-mf` (`mf` を含む) |

`default-*` で選ばれていないバックエンドでも、feature が有効なら明示 API で利用できます。

- 列挙: `VideoDeviceList::enumerate_avf()` / `enumerate_v4l2()` / `enumerate_pipewire()` / `enumerate_mf()`
- キャプチャ: `VideoCapture::new_avf()` / `new_v4l2()` / `new_pipewire()` / `new_mf()`

`default-*` を 1 つも有効にしていないプラットフォームでは、`VideoCapture::new()` と `VideoDeviceList::enumerate()` は存在しません。明示 API を使ってください。
バックエンドを 1 つも有効にしていないプラットフォームでは、`VideoCapture` / `VideoDevice` / `VideoDeviceList` は公開されません。

Linux では `v4l2` と `pipewire` を同時に有効化できます。
両方有効なとき、既定は `default-v4l2` または `default-pipewire` のどちらか一方だけを指定します。

### `mjpeg` feature

Linux (V4L2) で MJPEG フォーマットのパススルーキャプチャを利用可能にする feature です。
`pixel_format = Some(PixelFormat::Mjpeg)` を指定することで、MJPEG カメラから JPEG ペイロードを直接受け取れます。
V4L2 バックエンドが必要なため、`mjpeg` feature は自動的に `v4l2` を有効化します。

MJPEG 対応外の環境 (macOS / Windows / PipeWire) で `PixelFormat::Mjpeg` を指定すると `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` を返します。

```bash
# MJPEG 対応を有効化してビルド
cargo build -p shiguredo_video_device --features mjpeg
```

## 対応ピクセルフォーマット

| `PixelFormat` | 説明 | 備考 |
| --- | --- | --- |
| `Nv12` | YUV 4:2:0 semi-planar | |
| `Yuy2` | YUV 4:2:2 packed | |
| `I420` | YUV 4:2:0 planar | |
| `Mjpeg` | Motion JPEG (圧縮 JPEG ペイロード) | V4L2 + `mjpeg` feature のみ。デコードは利用者の責務 |
| `Unknown(u32)` | 未知の FourCC | キャプチャ要求には使えません |

ネゴシエーション結果が未知の FourCC になった場合、そのフレームはコールバックに渡りません。

## ビルド要件

### macOS

Xcode Command Line Tools が必要です。
ビルド時に bindgen が Xcode Command Line Tools に含まれる libclang を利用します。

```bash
xcode-select --install
```

### Ubuntu (Linux)

V4L2 バックエンド (デフォルト):

ビルド時に bindgen が libclang を利用するため、`libclang-dev` が必要です。

```bash
sudo apt install libclang-dev
```

ビデオデバイスへのアクセス権限については [docs/LINUX.md](docs/LINUX.md) を参照してください。

PipeWire バックエンド:

```bash
sudo apt install libclang-dev libpipewire-0.3-dev pkg-config
```

PipeWire バックエンドは `libpipewire-0.3` に動的リンクします。
実行時には `libpipewire-0.3` の共有ライブラリと、動作中の PipeWire デーモンが必要です。

### Windows

追加の依存パッケージは不要です。

## ビルド

```bash
# デフォルト (macOS AVFoundation / Linux V4L2 / Windows Media Foundation)
cargo build -p shiguredo_video_device

# Linux で既定バックエンドを PipeWire にする
#（デフォルトで default-v4l2 が有効なので、--no-default-features が必要）
cargo build -p shiguredo_video_device --no-default-features --features default-pipewire

# Linux で V4L2 と PipeWire の両方を有効化し、既定は V4L2 のままにする
cargo build -p shiguredo_video_device --features pipewire
```

## 使い方

### デバイス列挙

```rust
use shiguredo_video_device::VideoDeviceList;

// デバイス一覧を取得 (既定バックエンド)
let device_list = VideoDeviceList::enumerate()?;
for device in &device_list {
    println!("デバイス: {} (ID: {})", device.name()?, device.unique_id()?);

    // 対応フォーマット一覧
    for format in device.formats() {
        println!(
            "  {}x{} {:.0}-{:.0}fps {}",
            format.width, format.height,
            format.min_fps, format.max_fps,
            format.pixel_format.name()
        );
    }
}
```

### キャプチャ

```rust
use shiguredo_video_device::{VideoCapture, VideoCaptureConfig};

// キャプチャ設定
let config = VideoCaptureConfig {
    device_id: None, // デフォルトデバイスを使用
    width: 1280,
    height: 720,
    fps: 30,
    pixel_format: None, // 未指定ならデフォルト選択。Some(shiguredo_video_device::PixelFormat::Yuy2) のように指定可能
};

// コールバックでフレームを受信 (既定バックエンド)
let mut capture = VideoCapture::new(config, |frame| {
    println!(
        "フレーム: {}x{} {} timestamp={}us",
        frame.width, frame.height,
        frame.pixel_format.name(),
        frame.timestamp_us
    );
})?;

// キャプチャ開始
capture.start()?;

// ... キャプチャ中 ...

// キャプチャ停止
capture.stop();
```

`VideoCaptureConfig` の注意点:

- Windows では `width` / `height` / `fps` は正の整数である必要があります。違反すると `Error::InvalidCaptureConfig` を返します
- `pixel_format: None` の場合はバックエンドがデフォルト選択します

キャプチャのライフサイクル:

- `VideoCapture::new()` / `new_*()` はセッションを作るだけです。キャプチャは `start()` を呼ぶまで始まりません
- `start()` は冪等です。既に running なら `Ok(())` を返します
- macOS ではセッションが開始するまで、PipeWire ではストリーミング状態になるまで `start()` がブロックします
- `stop()` はブロッキングです。キャプチャスレッド / コールバックの完了を待ってから復帰します。running でなければ何もしません
- `stop()` 後の再 `start()` は許容します
- `VideoCapture` を drop すると、running なら `stop()` と同じブロッキング停止をしてからセッションを破棄します
- Windows の `VideoCapture` は `Send` ではありません。作成したスレッドで使い、そのスレッドで drop してください

フレームコールバックの制約:

- コールバックに渡される `VideoFrame` のスライスは、その呼び出し中にのみ有効です。呼び出し後も保持する場合は `VideoFrame::to_owned()` でコピーしてください
- コールバック内から `stop()` を呼んだり `VideoCapture` を drop したりしないでください。macOS ではデッドロックします。Linux (V4L2 / PipeWire) では `stop()` がコールバックの完了を待たずに戻るため、drop すると解放済みのメモリを参照します。停止はチャネルやフラグで別スレッドに通知し、そのスレッドで `stop()` を呼んでください
- コールバックは panic してはなりません。panic した場合、macOS / Linux ではプロセスが abort することがあります。Windows ではキャプチャスレッドが終了し、`stop()` の後の再 `start()` が `Error::CaptureFaulted` を返します。`CaptureFaulted` になった場合は、同じ `VideoCapture` を再利用せず新しく構築し直してください

macOS のカメラ権限:

- カメラ権限が未決定の場合、`VideoCapture::new()` / `new_avf()` が OS の許可ダイアログを表示し、応答があるまで戻りません
- 権限が拒否または制限されている場合は `Error::SessionCreateFailed` を返します
- アプリバンドルとして配布する場合は、`Info.plist` に `NSCameraUsageDescription` キーが必要です

## サンプル

### デバイス一覧

デバイス一覧を JSON で出力します。

```bash
cargo run --example device_list
```

### デバイス情報

デバイスごとのフォーマット詳細と統計情報を JSON で出力します。

```bash
cargo run --example device_info
```

### カメラプレビュー

カメラ映像をキャプチャして raw-player でプレビュー表示します。
Esc キーで終了し、S キーで統計オーバーレイの表示を切り替えます。

```bash
cargo run --example camera_preview -- --list-devices
cargo run --example camera_preview
cargo run --example camera_preview -- --resolution 1080p --fps 60
cargo run --example camera_preview -- --video-device-id <デバイス ID> --duration 10
```

| オプション | 説明 | 既定値 |
| --- | --- | --- |
| `--list-devices` | デバイス一覧を表示して終了する | |
| `--video-device-id <ID>` | キャプチャするデバイスの ID (`--list-devices` が表示する `ID`) | バックエンドの既定デバイス |
| `--resolution <RES>` | 解像度。`4k` / `2160p` / `1080p` / `720p` / `540p` または `1280x720` のような `WxH` 形式 | `720p` |
| `--fps <FPS>` | フレームレート | `30` |
| `--duration <SEC>` | 再生時間 (秒) | 無制限 |

## Agent Skills

[Agent Skills](https://agentskills.io/) 形式のスキルを同梱しています。`gh skill install` コマンドで対応する AI エージェント (Claude Code, Cursor, GitHub Copilot, Gemini CLI 等) にインストールでき、エージェントがこのライブラリの API やバックエンドごとの挙動差を理解した上で支援できるようになります。

```bash
gh skill install shiguredo/video-device-rs shiguredo-video-device
```

スキルの内容は [skills/shiguredo-video-device/SKILL.md](https://github.com/shiguredo/video-device-rs/blob/develop/skills/shiguredo-video-device/SKILL.md) を参照してください。

## ライセンス

Apache License 2.0

```text
Copyright 2026-2026, Shiguredo Inc.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
