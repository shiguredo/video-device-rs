# PipeWire のフレーム長契約が C と Rust で一致せず、複数プレーンの SPA バッファも扱えない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-frame-buffer-contract
- Polished: {YYYY-MM-DD}

## 目的

`src/video_pipewire.c` の `on_process` が検証する必要バイト数と、`src/capture_ffi.rs` の `frame_callback` が作るスライス長が一致していない。また、SPA バッファの `n_datas > 1`（複数プレーン）構成を扱えていない。

1. YUY2: C 側の `required_bytes_yuy2` は `(height - 1) * stride + width * 2` を検証するが、Rust 側の `yuy2_packed_frame_bytes` は `stride * height` をスライス長にする。`stride > width * 2` のとき Rust は検証済み領域より最大 `stride - width * 2` バイト多く読む
2. NV12 / I420: SPA の `spa_buffer` は `n_datas` 個のデータブロックを持ち、各ブロックが映像プレーンになりうる（PipeWire 公式ドキュメント「SPA Buffers」）。現実装は `datas[0]` の `data` / `maxsize` / `chunk` しか参照せず、UV を `plane + stride * height` で計算する。`n_datas > 1` の producer では、必要長が `datas[0].maxsize` を超えて全フレームが無音で破棄されるか、UV ポインタが誤る
3. `chunk->stride == 0` のフレームを一律破棄しており、公式 example が持つ「stride 0 のとき size / height から補う」フォールバックが無い
4. `SPA_META_VideoCrop` を無視しているため、クロップメタ付きバッファでは報告する `width` / `height` と実データがずれる
5. `on_process` の `(stride + 1) / 2` は `stride == INT32_MAX` で符号付き整数オーバーフロー（未定義動作）になる

## 優先度根拠

- High。YUY2 は確保済み領域外の読み出し（未定義動作）に発展しうる
- 複数プレーン構成では「start 成功・フレーム 0 枚」の無音故障になり、macOS では実機検証できない Linux 側の主要経路
- `/review-code` の重要指摘として 3 周にわたり確認済み

## 現状

C 側の検証は次の式のみで、Rust 側のスライス長と一致しない。

```c
return add_u64_ov(rows, w2, out);   /* (h-1)*stride + w*2 */
```

`on_process` は `spa_buf->n_datas` を確認せず、`spa_buf->datas[0]` だけを使う。PipeWire の `video-play.c` example は `n_datas == 1` とそれ以外で分岐し `datas[1]` / `datas[2]` を使う。

## 設計方針

- C と Rust の長さ計算を一元化する。C 側の検証を Rust のスライス長（`stride * height`）に合わせるか、C からプレーンごとの実バイト長を FFI で渡して Rust の再計算を廃止する
- `n_datas` を見て単一プレーン（連続）と複数プレーンを分岐する。複数プレーンでは `datas[i]` の `data` / `chunk->stride` / `maxsize` をプレーンごとに検証する
- `chunk->stride == 0` 時のフォールバックを検討する
- 未対応構成ではコールバックを呼ばず、`start()` または初回フレームで利用者へ通知する

## 完了条件

- YUY2 / NV12 / I420 の全経路で、C の検証済み長が Rust のスライス長以上であることがテストで固定されている
- `n_datas > 1` を模したバッファでフレームが正しく構築される（または明示的にエラー通知される）
- ASAN 有効ビルドで境界外読み出しが検出されない
- Linux 実機の device test で NV12 / I420 / YUY2 のフレーム受信が確認できる

## 解決方法

{完了時に記入}
