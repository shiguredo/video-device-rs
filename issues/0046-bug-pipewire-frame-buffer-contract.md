# PipeWire のフレームバッファ契約の不整合と複数プレーン・stride 0・VideoCrop・整数オーバーフローの不備

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-frame-buffer-contract
- Polished: 2026-11-05

## 目的

`src/video_pipewire.c` の `on_process` が検証する必要バイト数と、`src/capture_ffi.rs` の `frame_callback` が作るスライス長が一致していない。また、SPA バッファの `n_datas > 1`（複数プレーン）構成を扱えていない。

1. YUY2: C 側の `required_bytes_yuy2` は `(height - 1) * stride + width * 2` を検証するが、Rust 側の `yuy2_packed_frame_bytes` は `stride * height` をスライス長にする。`stride > width * 2` のとき Rust は検証済み領域より最大 `stride - width * 2` バイト多く読む。C の `maxsize` 検証を通過しても、Rust のスライスが確保領域を超えうる
2. NV12 / I420: SPA の `spa_buffer` は `n_datas` 個のデータブロックを持ち、各ブロックが映像プレーンになりうる（PipeWire 公式ドキュメント「SPA Buffers」）。現実装は `datas[0]` の `data` / `maxsize` / `chunk` しか参照せず、UV を `plane + stride * height` で計算する。`n_datas > 1` の producer では、必要長が `datas[0].maxsize` を超えて全フレームが無音で破棄されるか、UV ポインタが誤る。I420 の 3 プレーンは、既存の `FrameCallback` が `uv_data` を U+V 連結として扱う契約とも合わない
3. `chunk->stride == 0` のフレームを一律破棄しており、公式 example が持つ「stride 0 のとき size / height から補う」フォールバックが無い
4. `SPA_META_VideoCrop` を要求しておらず、クロップメタ付きバッファでも `width` / `height` はネゴシエートされたバッファ寸法（クロップ前）のままになる。`VideoFrame` の `width` / `height` がバッファ寸法であり、クロップ領域は公開 API で扱わないことを doc に明記する（クロップ対応は別 issue）
5. `on_process` の `(stride + 1) / 2` は `stride == INT32_MAX` で符号付き整数オーバーフロー（未定義動作）になる

## 優先度根拠

- High。YUY2 は確保済み領域外の読み出し（未定義動作）に発展しうる
- 複数プレーン構成では「start 成功・フレーム 0 枚」の無音故障になり、macOS では実機検証できない Linux 側の主要経路
- `/review-code` の重要指摘として 3 周にわたり確認済み
- VideoCrop と整数オーバーフローは優先度は低いが、同じ `on_process` のバッファ契約なので本 issue でまとめて扱う

## 現状

C 側の YUY2 の検証は次の式のみで、Rust 側のスライス長と一致しない。

```c
return add_u64_ov(rows, w2, out);   /* (h-1)*stride + w*2 */
```

`on_process` は `spa_buf->n_datas == 0` と `spa_buf->datas[0].data` の NULL を確認するだけで、`n_datas > 1` の分岐を持たず、`spa_buf->datas[0]` だけを使う。PipeWire の `video-play.c` example は `n_datas == 1` とそれ以外で分岐し `datas[1]` / `datas[2]` を使う。`chunk->stride == 0` のときは `chunk->size / height` で補うフォールバックを持つ。

`SPA_META_VideoCrop` は `pw_stream_connect` のパラメータに含めておらず、`on_process` でも参照していない。`VideoFrame` の `width` / `height` はバッファ寸法として扱われ、クロップ領域は公開 API で表現されない。

`on_process` の `(stride + 1) / 2` は `stride` が `int32_t` のまま計算されるため、`stride == INT32_MAX` で `stride + 1` が符号付き整数オーバーフロー（未定義動作）になる。

## 設計方針

- C と Rust の長さ契約を整合させる。原則として C 側の検証長を Rust のスライス長以上にそろえる方法を採用し、`FrameCallback` の引数追加は行わない。YUY2 は `stride >= width * 2` を確認したうえで `stride * height` を検証する。複数プレーンの場合はプレーンごとの必要長を検証する。C 側だけでは契約を保証できない場合に限り、`FrameCallback` に実バイト長を追加する（その場合は FFI バックエンド全体に影響する）
- `maxsize` に対する安全確認は Rust が `from_raw_parts` に渡すスライス長で行い、`chunk->size` に対する有効データ確認はフォーマットごとの有効長で行う。両者を混同しない。複数プレーンの場合はプレーンごとに `data` / `chunk->offset` / `chunk->stride` / `chunk->size` / `maxsize` を検証し、`chunk->offset` を各プレーンの先頭ポインタに適用する
- `n_datas == 1`（連続プレーン）は現行どおり `datas[0]` の Y/UV 連続領域を使う。NV12 は `n_datas` が 1 または 2、I420 は 1 / 2 / 3、YUY2 は 1 のみを扱い、それ以外は想定外とする。NV12 の `n_datas == 2` は `datas[0]` を Y、`datas[1]` を UV として扱い、UV の stride は `datas[1].chunk->stride` を使う。I420 の `n_datas == 2` は `datas[1]` を U+V 連結としてそのまま `uv_data` に渡す。I420 の `n_datas == 3` は `datas[1]` の U と `datas[2]` の V を、AVF と同様に共通 stride（U/V の最大値）の連結一時バッファへコピーしてからコールバックする。`FrameCallback` の引数は変更しない
- `chunk->stride == 0` のときは、データブロックが次のいずれか（YUY2、NV12 の UV、I420 の分離 U/V、I420 の U+V 連結）の場合に `chunk->size / プレーンの高さ` で stride を補う。プレーンの高さは YUY2 が `height`、NV12 の UV と I420 の分離 U/V が `height.div_ceil(2)`、I420 の U+V 連結が `2 * height.div_ceil(2)`。連続 Y+UV ブロック（NV12/I420 の `n_datas == 1`）では `chunk->size / height` が Y stride にならないため、補わずにフレームを破棄する。補った stride が 0 以下、または幅要件を満たさない場合はフレームを破棄する。負の stride は従来どおり不正として破棄する
- `(stride + 1) / 2` は `((int64_t)stride + 1) / 2` で計算する。計算結果が `int32_t` の範囲を超える場合はフレームを破棄する
- `SPA_META_VideoCrop` は本 issue では扱わない。`width` / `height` はネゴシエートされたバッファ寸法と定義し、クロップ領域は公開 API で表現しないことを `VideoFrame` の doc に明記する。クロップ対応が必要になった場合は別 issue とする
- 未対応構成（想定外の `n_datas`、stride フォールバック失敗、計算結果の範囲外など）ではコールバックを呼ばず、初回のみ stderr に英語のログを出してフレームを破棄する。`start()` の時点で判定できる未対応（未知フォーマットなど）は別 issue の範囲とし、本 issue では `start()` を変更しない
- 主な変更対象は `src/video_pipewire.c`、`src/video_pipewire.h`（テスト用の長さ計算・バッファ検証ヘルパの公開）、`tests/test_pipewire.rs`、`src/types.rs`（`VideoFrame` の doc）。`src/frame_math.rs` は原則として長さ計算自体を変更しないが、C と Rust の比較テストを追加する場合は `#[cfg(test)]` モジュールだけを追加する（FFI に実バイト長を追加する場合を除く）

## 完了条件

- YUY2 / NV12 / I420 の全経路で、C が `maxsize` に対して検証する長さが、Rust が `from_raw_parts` に渡す各スライス長以上であることをテストで固定する。C 側の長さ計算を `video_pipewire.h` に公開するなどして、Rust のテストから C と Rust の値を突き合わせられるようにする
- NV12 の `n_datas == 2`、I420 の `n_datas == 2`、I420 の `n_datas == 3` を模したバッファで、フレームが正しく構築されることをテストで固定する。テストのために `on_process` のバッファ検証部分を切り出すか、`spa_buffer` を組み立てる C のテストヘルパを追加する。`n_datas` が想定外の場合はコールバックを呼ばず、初回のみ stderr に英語ログが出ることを確認する
- `chunk->stride == 0` のバッファで、単一データブロックで stride を一意に決められるレイアウトでは `chunk->size / プレーンの高さ` から stride を補えること、連続 Y+UV ブロックではフレームが破棄されることをテストで固定する
- `stride == INT32_MAX` の I420 で `(stride + 1) / 2` がオーバーフローしないことをテストで固定する
- `VideoFrame` の doc に、`width` / `height` がネゴシエートされたバッファ寸法であり、`SPA_META_VideoCrop` のクロップ領域は公開 API で扱わないことを明記する
- ASAN 有効ビルド（`CFLAGS` などで C 側を `-fsanitize=address` でビルド）で境界外読み出しが検出されないこと。CI には ASAN ジョブが無いため手動確認とする。必要に応じて `ASAN_OPTIONS=detect_leaks=0` を付ける
- Linux 実機の device test で、`tests/test_pipewire.rs` に NV12 / I420 / YUY2 のキャプチャテストを追加し、デバイスの `formats()` に対象フォーマットが含まれる場合だけフレーム受信を確認する。未対応フォーマットはスキップする
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する
- CI の pipewire ビルド・テストジョブ（`--no-default-features --features default-pipewire` と `--features v4l2,pipewire`）が通る

## 解決方法

{完了時に記入}
