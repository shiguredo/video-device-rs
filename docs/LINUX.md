# Linux

## パーミッション

ビデオデバイスにアクセスするには、実行ユーザーが `video` グループに所属している必要があります。

```bash
sudo usermod -aG video <ユーザー名>
```

反映にはログアウト後の再ログインが必要です。即時反映する場合は次を実行してください。

```bash
newgrp video
```

## feature

Linux では次の feature を使います。

| feature | 意味 |
| --- | --- |
| `v4l2` | V4L2 バックエンドを有効化する |
| `pipewire` | PipeWire バックエンドを有効化する |
| `default-v4l2` | 既定バックエンドを V4L2 にする (`v4l2` を含む) |
| `default-pipewire` | 既定バックエンドを PipeWire にする (`pipewire` を含む) |
| `mjpeg` | V4L2 で MJPEG パススルーキャプチャを有効化する (`v4l2` を含む) |

クレートのデフォルト feature には `default-v4l2` が含まれます。
`default-v4l2` と `default-pipewire` を同時に指定するとコンパイルエラーになります。
`v4l2` と `pipewire` 自体は同時に有効化できます。

既定以外のバックエンドも、feature が有効なら明示 API で利用できます。

- 列挙: `VideoDeviceList::enumerate_v4l2()` / `enumerate_pipewire()`
- キャプチャ: `VideoCapture::new_v4l2()` / `new_pipewire()`

## V4L2

デフォルトでは V4L2 が既定バックエンドです。追加の依存パッケージは不要です。

```bash
cargo run --example device_list
```

## PipeWire

### ビルド依存

PipeWire バックエンドのビルドには `libpipewire-0.3-dev` が必要です。

```bash
sudo apt install libpipewire-0.3-dev
```

### 既定バックエンドを PipeWire にする

デフォルトで `default-v4l2` が有効なため、`--no-default-features` と `default-pipewire` を指定します。

```bash
cargo run --no-default-features --features default-pipewire --example device_list
```

### V4L2 と PipeWire を併存させる

既定は V4L2 のまま、PipeWire も有効化できます。

```bash
cargo run --features pipewire --example device_list
```

この場合、既定 API (`VideoDeviceList::enumerate()` / `VideoCapture::new()`) は V4L2 を使い、PipeWire は `enumerate_pipewire()` / `new_pipewire()` で利用します。

### 注意点

- PipeWire のカメラ認識は環境によって不安定な場合があります
- V4L2 デバイスが PipeWire に認識されない場合は、OS の再起動や `pipewire-v4l2` の導入が必要なことがあります
- `wpctl status` の Video セクションにデバイスが表示されることを確認してください
- 安定性を重視する場合は V4L2 バックエンド (デフォルト) の利用を推奨します

## MJPEG

Linux (V4L2) で MJPEG フォーマットのパススルーキャプチャを使う場合は `mjpeg` feature を有効化します。

```bash
cargo build -p shiguredo_video_device --features mjpeg
```

キャプチャ時は `pixel_format = Some(PixelFormat::Mjpeg)` を指定してください。
PipeWire バックエンドでは MJPEG を利用できません。
