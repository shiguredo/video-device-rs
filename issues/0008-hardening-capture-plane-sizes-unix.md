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
- UV（連結 U+V）: `(stride_uv as usize).checked_mul(height as usize)?`（現行 163〜164 行の意図と同じだが `checked_mul`）

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

- [ ] 上記ヘルパが `frame_callback` から呼ばれ、負ストライド等で `from_raw_parts` に到達しない。
- [ ] `cargo test` が通る（Linux/macOS ビルド対象）。
- [ ] 新規コメントは日本語。

## 検討結果

- **調査**: 現行実装は `stride` 負値と乗算オーバーフローに弱い。ヘルパで `checked_mul` と非正ストライドの拒否を一本化すれば、`frame_callback` の分岐は読みやすい。
- **本 issue だけで修正可能**: はい。`src/capture.rs` のみ変更すればよい（`lib.rs` のモジュール構成は変えない）。
