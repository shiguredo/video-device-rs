# examples

## device_list

デバイス一覧を JSON で出力する。

```bash
cargo run --example device_list
```

## device_info

デバイスごとのフォーマット詳細と統計情報を JSON で出力する。

```bash
cargo run --example device_info
```

## camera_preview

カメラ映像をキャプチャして raw-player でプレビュー表示する。

```bash
cargo run --example camera_preview -- --list-devices
cargo run --example camera_preview
cargo run --example camera_preview -- --resolution 1080p --fps 60
```
