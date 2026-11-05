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
Esc キーで終了し、S キーで統計オーバーレイの表示を切り替える。

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
