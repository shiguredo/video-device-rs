# Unix `capture.rs` のフレームコールバックでストライド・プレーン長を検証する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

`extern "C" fn frame_callback` 内の `std::slice::from_raw_parts` が、**負のストライド**や **`usize` 乗算のオーバーフロー**で危険な長さにならないようにする（**セグフォ／未定義動作**の抑止）。

## 現状調査（この issue 作成時点の `src/capture.rs`）

| 行付近 | 問題 |
|--------|------|
| 136〜157 NV12 | `stride` / `stride_uv` が負のとき `(stride as usize) * …` が巨大になりうる。乗算に `checked_mul` が無い。 |
| 158〜179 I420 | 同様。 |
| 181〜184 YUY2 | `data_size = (stride as usize) * (height as usize)` のみ。`stride <= 0` の早期 return が無い。 |
| 全体 | ネイティブが渡すバッファの**実長**は Rust からは分からない。本変更は**ストライド・高さの組み合わせとして不自然な長さを作らない**下限防御。C との式合わせは **0004**。 |

## 実装する場所

- ファイル: **`src/capture.rs`**
- **`unsafe impl Sync for VideoCapture {}` の直後（約 109 行の後）**から **`extern "C" fn frame_callback` の直前**に、非公開関数 3 つを置く。
- **`frame_callback` 本体**は約 111〜201 行を、下記ロジックに差し替える（分岐構造は維持）。

## 非公開ヘルパ（仕様）

名前は任意だが、以下の意味に合わせる。

### `nv12_plane_sizes(stride, stride_uv, height) -> Option<(usize, usize)>`

- `stride <= 0 || stride_uv <= 0 || height <= 0` → `None`。
- Y バイト数: `(stride as usize).checked_mul(height as usize)?`
- UV バイト数: `let uv_h = (height as usize).div_ceil(2);` とし `(stride_uv as usize).checked_mul(uv_h)?`
- 戻り値 `(y_size, uv_size)`。

### `i420_plane_sizes(stride, stride_uv, height) -> Option<(usize, usize)>`

- 上記と同様にストライド・高さの非正を弾く。
- Y: `(stride as usize).checked_mul(height as usize)?`
- UV（連結 U+V）: issue 作成時の草案は `stride_uv * height`（`checked_mul`）だったが、**下記「問題解決」のとおり 0004（macOS C）に合わせた式で解決した。**

### `yuy2_packed_frame_bytes(stride, height) -> Option<usize>`

- `stride <= 0 || height <= 0` → `None`。
- `(stride as usize).checked_mul(height as usize)`

## `frame_callback` 内の振る舞い

1. 既存どおり `user_data` / `data` の null、`width` / `height` の非正は return。
2. **NV12**: `uv_data` が null なら return。`nv12_plane_sizes` が `None` なら return。`y_size` / `uv_size` で `from_raw_parts(data, y_size)` と `from_raw_parts(uv_data, uv_size)`。
3. **I420**: 同様に `i420_plane_sizes`。
4. **YUY2**: `yuy2_packed_frame_bytes` が `None` なら return。`from_raw_parts(data, data_size)` の `data_size` のみ使用。

## 関連 issue

- **0004**: C 実装との**バイト長一致**の検証（本変更は Rust 側のガードのみ）。
- **0005**: 上記ヘルパの**単体テスト**（`#[cfg(test)] mod tests` を `capture.rs` 末尾に追加する方法可）。

## 完了条件（チェックリスト）

- [x] 上記ヘルパが `frame_callback` から呼ばれ、負ストライド等で `from_raw_parts` に到達しない。
- [x] `cargo test` が通る（Linux/macOS ビルド対象）。
- [x] 新規コメントは日本語。

## 完了条件の検証

2026-04-02 に `capture.rs` と `cargo test` で確認した。

- `frame_callback` が `nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes` 経由で長さを決め、`None` や null で早期 return している（約 149 行以降）。
- ヘルパと `frame_callback` 付近のコメントは日本語。
- `cargo test` 全成功。

## 検討結果

- **調査**: 現行実装は `stride` 負値と乗算オーバーフローに弱い。ヘルパで `checked_mul` と非正ストライドの拒否を一本化すれば、`frame_callback` の分岐は読みやすい。
- **本 issue だけで修正可能**: はい。`src/capture.rs` のみ変更すればよい（`lib.rs` のモジュール構成は変えない）。

## 問題解決

### 問題だったこと

1. **負のストライド・オーバーフロー**: `stride` や `stride_uv` が負のとき `as usize` の乗算が巨大になり、`from_raw_parts` が危険な長さになりうる。YUY2 は `stride <= 0` の早期 return がなかった。
2. **I420 の UV バイト数**: 本 issue の「非公開ヘルパ（仕様）」では UV を **`stride_uv * height`** とする草案だったが、**0004** で macOS C と突き合わせると奇数 `height` で **C の `uvSize` と一致しない**。

### どう解決したか

1. `nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes` を追加し、非正のストライド・高さは `None`、`checked_mul` で乗算。`frame_callback` は `None` のとき return し、**不自然な長さで `from_raw_parts` しない**。
2. I420 の UV は **0004 と同じ式**にした: `chroma_h = (height + 1) / 2`（usize）、`uv = stride_uv * chroma_h * 2`（各段 `checked_mul`）。NV12・YUY2 は本文の「非公開ヘルパ（仕様）」どおり。

### issue 本文との差異

| 項目 | issue 本文 | 実際の解決 |
|------|------------|------------|
| I420 の UV 長 | 草案: `stride_uv * height`（`checked_mul`） | **0004** に合わせ `stride_uv * ((height + 1) / 2) * 2` に変更（macOS `uvSize` と一致） |
| その他 | NV12 / YUY2 のヘルパ仕様 | **本文どおり** |

0004 との役割分担: **0008** は Rust 側のガード、**0004** は C との式の根拠とコメント（`video_c.m` / `video_v4l2.c` / `capture.rs` doc）。
