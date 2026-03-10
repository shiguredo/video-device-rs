# video-device-rs

[![shiguredo_video_device](https://img.shields.io/crates/v/shiguredo_video_device.svg)](https://crates.io/crates/shiguredo_video_device)
[![Documentation](https://docs.rs/shiguredo_video_device/badge.svg)](https://docs.rs/shiguredo_video_device)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

> [!WARNING]
> このライブラリは開発中であり、仕様が積極的に変更される場合があります。

## About Shiguredo's open source software

We will not respond to PRs or issues that have not been discussed on Discord. Also, Discord is only available in Japanese.

Please read <https://github.com/shiguredo/oss> before use.

## 時雨堂のオープンソースソフトウェアについて

利用前に <https://github.com/shiguredo/oss> をお読みください。

## 概要

macOS / Linux / Windows に対応したビデオデバイスライブラリです。

## 対応プラットフォーム

- macOS: AVFoundation
- Linux: V4L2 (デフォルト) / PipeWire
- Windows: Media Foundation

## Linux の feature

Linux では `v4l2` (デフォルト) と `pipewire` の 2 つの feature を選択できます。両方を同時に指定することはできません。

## ビルド要件

### macOS

追加の依存パッケージは不要です。

### Ubuntu (Linux)

V4L2 バックエンド (デフォルト):

追加の依存パッケージは不要です。

PipeWire バックエンド:

```bash
sudo apt install libpipewire-0.3-dev
```

### Windows

追加の依存パッケージは不要です。

## ビルド

```bash
# デフォルト (macOS / Linux V4L2 / Windows)
cargo build -p shiguredo_video_device

# Linux PipeWire バックエンド
cargo build -p shiguredo_video_device --no-default-features --features pipewire
```

## 使い方

### デバイス列挙

```rust
use shiguredo_video_device::VideoDeviceList;

// デバイス一覧を取得
let device_list = VideoDeviceList::enumerate()?;
for device in device_list.devices() {
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

// コールバックでフレームを受信
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
