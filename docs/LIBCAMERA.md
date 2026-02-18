# libcamera

libcamera のバインディングは `shiguredo_libcamera` クレートとして提供しています。

ビルド時に C++ ラッパー (`wrapper/`) をコンパイルし、bindgen で FFI バインディングを生成した上で、安全な Rust API を提供します。

## 対応環境

Raspberry Pi が開発している [libcamera](https://github.com/raspberrypi/libcamera) を利用しています。
Raspberry Pi OS 等の ARM64 Linux 環境を対象としています。

## ビルド要件

libcamera の開発用パッケージが必要です。

```bash
sudo apt install libcamera-dev
```

## ビルド

```bash
cargo build -p shiguredo_libcamera
```

## サンプル

```bash
# カメラ一覧の表示
cargo run -p shiguredo_libcamera --example list_cameras

# キャプチャ
cargo run -p shiguredo_libcamera --example capture

# コントロール情報の表示
cargo run -p shiguredo_libcamera --example controls
```
