# V4L2 が bytesperline を無視して width を stride として使用する

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-bytesperline-stride
- Polished: {YYYY-MM-DD}

## 目的

V4L2 バックエンドが `VIDIOC_S_FMT` 後にドライバが実際に設定した `bytesperline` を保存せず、`width` を stride として使用している。DMA アラインメントにより `bytesperline != width` となる環境で映像が崩壊するバグを修正する。

## 優先度根拠

- High。映像データが崩壊する致命的なバグ
- 組み込みカメラ・ハードウェアアクセラレータ付きキャプチャデバイスでは `bytesperline != width` が一般的（64, 128, 256 バイト境界アラインメント）
- UVC カメラ（USB）では `bytesperline == width` の場合が多く表面化しにくいが、Linux バックエンドとして実用上致命的
- `/review-code` の致命的指摘として確認

## 現状

`src/video_v4l2.c:506-508` で `VIDIOC_S_FMT` 後に保存しているのは `width` と `height` のみ:

```c
session->width = fmt.fmt.pix.width;
session->height = fmt.fmt.pix.height;
session->pixel_format = pixel_format;
```

`VideoSession` 構造体（`src/video_v4l2.c:45-56`）に `bytesperline` フィールドが存在しない。

`capture_thread`（`src/video_v4l2.c:538-654`）では全フォーマットで `session->width` を stride として使用:

- NV12（:597）: `y_size = width * height`、stride 引数 = `session->width`
- I420（:612）: `y_size = width * height`、stride 引数 = `session->width`
- YUY2（:626）: `need = width * 2 * height`、stride 引数 = `session->width * 2`

例: `width=640, bytesperline=768` の場合、実際の Y プレーンは `768 * height` バイトだが、コードは `640 * height` バイトと計算する。`uv_data = data + 640 * height` は Y プレーンのパディング途中を指し、UV プレーンのオフセットがずれて映像が崩壊する。

## 設計方針

1. `VideoSession` 構造体に `int bytesperline;` フィールドを追加する
2. `video_v4l2_session_create` で `VIDIOC_S_FMT` 後に `fmt.fmt.pix.bytesperline` を保存する
3. `capture_thread` 内で stride として `session->width` の代わりに `session->bytesperline` を使用する
4. NV12: `y_size = bytesperline * height`、stride 引数 = `bytesperline`、UV stride も `bytesperline`
5. I420: `y_size = bytesperline * height`、stride 引数 = `bytesperline`、`stride_uv = (bytesperline + 1) / 2`
6. YUY2: `need = bytesperline * height`（bytesperline は既に width * 2 以上）、stride 引数 = `bytesperline`
7. MJPEG: bytesperline は無関係（ペイロード長は `buf.bytesused` から取得）。変更なし

### 採用しない案

- Rust 側で stride を補正する: C 側で正しい値を渡すべきであり、Rust 側で推測するのは誤りの温床になる

## 完了条件

- `VideoSession` に `bytesperline` フィールドを追加し、`VIDIOC_S_FMT` 後に `fmt.fmt.pix.bytesperline` を保存する
- `capture_thread` の全フォーマット経路で stride として `bytesperline` を使用する
- `cargo build --workspace`（Linux、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
