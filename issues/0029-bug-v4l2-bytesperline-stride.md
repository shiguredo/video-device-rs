# V4L2 が bytesperline を保存せず width を stride として使用する

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-bytesperline-stride
- Polished: 2026-07-21

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

- NV12（:597）: `y_size = width * height`、（:600）`uv_size = width * uv_h`、stride 引数 = `session->width`、UV stride 引数 = `session->width`
- I420（:612）: `y_size = width * height`、（:614）`stride_uv = (width + 1) / 2`、（:615）`uv_total = stride_uv * chroma_h * 2`、stride 引数 = `session->width`
- YUY2（:626）: `need = width * 2 * height`、stride 引数 = `session->width * 2`

例: `width=640, bytesperline=768` の場合、実際の Y プレーンは `768 * height` バイトだが、コードは `640 * height` バイトと計算する。`uv_data = data + 640 * height` は Y プレーンのパディング途中を指し、UV プレーンのオフセットがずれて映像が崩壊する。

## 設計方針

1. `VideoSession` 構造体に `int bytesperline;` フィールドを追加する
2. `video_v4l2_session_create` で `VIDIOC_S_FMT` 後に `fmt.fmt.pix.bytesperline` を保存する
3. `capture_thread` 内で stride として `session->width` の代わりに `session->bytesperline` を使用する
4. NV12: `y_size = bytesperline * height`、`uv_size = bytesperline * uv_h`、`need = y_size + uv_size`、stride 引数 = `bytesperline`、UV stride 引数 = `bytesperline`（V4L2 NV12 では UV プレーンの stride も Y と同じ bytesperline になる）
5. I420: `y_size = bytesperline * height`、`stride_uv = (bytesperline + 1) / 2`（V4L2 の I420 は YUV 4:2:0 planar であり、UV 各プレーンの stride は Y の stride の半分。現行コードの `(width + 1) / 2` と同じ ceiling 除算を踏襲する。bytesperline はドライバがアラインメント済みで返すため通常偶数だが、奇数の場合も stride_uv が 1 バイト大きくなるだけで、`need > available` バウンズチェック（:617）によりオーバーリードは防止される。C の `need` 計算と Rust の `i420_plane_sizes` が同一式のため、`need <= available` なら Rust 側のスライスも `need` バイト以内に収まる）、`uv_total = stride_uv * chroma_h * 2`、`need = y_size + uv_total`、stride 引数 = `bytesperline`
6. YUY2: `need = bytesperline * height`（bytesperline は既に width * 2 以上）、stride 引数 = `bytesperline`
7. MJPEG: bytesperline は無関係（ペイロード長は `buf.bytesused` から取得）。変更なし
8. `bytesperline == 0` の防御: V4L2 仕様上 `VIDIOC_S_FMT` 後はドライバが有効な `bytesperline` を設定する義務があるが、不具合ドライバが 0 を返す可能性は排除できない。`bytesperline == 0` の場合は `width`（YUY2 なら `width * 2`）にフォールバックし、`fprintf(stderr, "warning: driver returned bytesperline=0, falling back to width-based stride\n")` で警告を出す。Rust 側（`frame_math`）は `stride <= 0` でフレームをドロップするためクラッシュはしないが、C 側でフォールバックすることで正常なキャプチャを維持する

### 変更不要な箇所

- **C 側の `available` バウンズチェック**: `capture_thread` 内の `need > available` チェックは変更不要。`available = min(bytesused, mmap_len)` であり、V4L2 仕様の `bytesused` は「バッファ内のデータが占めるバイト数」でパディングを含むかは仕様上の保証はない。実際には videobuf2 フレームワークを使うドライバが `bytesused = sizeimage`（パディング込み）を設定するため、正常系では bytesperline ベースの `need` は `available` を超えない。`bytesused` をパディングなしの実データ長で報告するドライバでは `need > available` となり全フレームが `goto requeue` で廃棄されるが、オーバーリードより安全であり、それはドライバ側の問題である
- **Rust 側（`capture_ffi.rs`、`frame_math.rs`、`types.rs`）**: C コールバックの stride 引数をそのまま使用するため変更不要。C 側が正しい `bytesperline` を stride として渡せば、`frame_math::nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes` は正しく動作する
- **`video.h` の FrameCallback 契約**: stride の意味（バイト/行）は不変。`data`/`uv_data` のレイアウト記述は stride の値に依存しない

### 影響範囲

- `VideoFrame.stride` の実効値が `width` から `bytesperline` に変わる。`bytesperline == width` の環境（UVC カメラ等）では変化なし。`bytesperline != width` の環境では現状が映像崩壊バグのため、修正後に `stride` が正しい値になる。バグ修正であり後方互換の問題はない
- デフォルト選択パス（`requested_pixel_format == 0`、:472-488）では NV12 → YUY2 の順で `VIDIOC_S_FMT` を試行するが、bytesperline の保存箇所（:506 以降）は両パス合流後であるため、追加の分岐は不要。最終的に成功した `fmt` から `fmt.fmt.pix.bytesperline` を取得する

### 採用しない案

- Rust 側で stride を補正する: C 側で正しい値を渡すべきであり、Rust 側で推測するのは誤りの温床になる
- `VIDIOC_G_FMT` で再取得する: `VIDIOC_S_FMT` 後の `fmt` 構造体にドライバが設定した値が既に入っているため、追加の ioctl は不要
- マルチプラン API（`V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE`）への移行: 本 issue のスコープを超える。シングルプラン API で bytesperline を正しく使えば NV12/I420/YUY2 は問題なく扱える

## 完了条件

- `VideoSession` に `bytesperline` フィールドを追加し、`VIDIOC_S_FMT` 後に `fmt.fmt.pix.bytesperline` を保存する
- `capture_thread` の全フォーマット経路で stride として `bytesperline` を使用する
- `cargo build --workspace`（Linux、default features）が通る（C コードは `enable_v4l2` 時のみコンパイルされるため、macOS 上では C の構文エラーは検出できない。Linux 環境での検証が必須）
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
