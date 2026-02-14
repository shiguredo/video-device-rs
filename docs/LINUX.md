# Linux

## パーミッション

ビデオデバイスにアクセスするには、実行ユーザーが `video` グループに所属している必要があります。

```bash
sudo usermod -aG video <ユーザー名>
```

反映にはログアウト→ログインが必要です。即時反映する場合は以下を実行してください。

```bash
newgrp video
```

## V4L2

デフォルトでは V4L2 バックエンドが使用されます。

```bash
cargo run --example device_list
```

## PipeWire

PipeWire バックエンドを使用するには `libpipewire-0.3-dev` と `pipewire-v4l2` が必要です。

```bash
sudo apt install libpipewire-0.3-dev pipewire-v4l2
```

`--no-default-features` で V4L2 を無効にし、`--features pipewire` を指定してください。

```bash
cargo run --no-default-features --features pipewire --example device_list
```

### 注意点

- PipeWire の V4L2 デバイス対応は環境によって不安定な場合があります
- V4L2 デバイスが PipeWire に認識されない場合は OS の再起動が必要なことがあります
- `wpctl status` の Video セクションにデバイスが表示されることを確認してください
- 安定性を重視する場合は V4L2 バックエンド（デフォルト）の使用を推奨します
