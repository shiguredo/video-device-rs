# キャプチャ用プレーン長計算の単体テストを追加する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

プレーン長計算の**回 regress 防止**（将来の変更で `from_raw_parts` に渡る長さが壊れるのを検知）。直接のランタイムバグ修正ではないが、**セグフォ経路の防御**を維持する。

## 対象ロジック

- ファイル: `src/capture.rs`
- 現在は非公開: `fn nv12_plane_sizes`, `fn i420_plane_sizes`, `fn yuy2_packed_frame_bytes`

現実装は `i32` を `usize` にキャストしてから `checked_mul` しており、**64 bit 環境では `i32::MAX` 級の積でも `usize` の範囲内なら `None` にならない**。そのため **「オーバーフローで必ず `None`」を期待するテストは不適切**（実装を歪める圧力になる）。

## 問題解決

- **回 regress 防止**: 方針 A（`capture.rs` 末尾の `#[cfg(test)] mod tests`）で、不正ストライド・正常系・奇数高さ・YUY2・**0004/0008 で確定した I420 式**（`i420_matches_macos_uv_formula`）をテストに固定した。
- issue 作成時の行番号メモは現行コードと一致しないため、参照はソースに従う。

## 実装方針（いずれか）

**A.** `src/capture.rs` 末尾に `#[cfg(test)] mod tests { ... }` を置き、上記関数を**同モジュールから**テストする（追加の `pub` 不要）。

**B.** 計算式だけを `src/plane_sizes.rs` 等に切り出して `pub(crate)` にし、`capture.rs` とテストの両方から使う。

## テストケースの中心（推奨）

| 観点 | 例 |
|------|-----|
| 不正ストライド・高さ | `stride <= 0`, `stride_uv <= 0`, `height <= 0` で `None` |
| 正常系の小さな値 | `nv12_plane_sizes(4, 4, 2)` の Y / UV バイト数 |
| 奇数高さ | `height` が奇数のときの UV 行数（`div_ceil(2)`）と期待する UV 長（issue 0004 の macOS I420 式と整合する値） |
| YUY2 | `yuy2_packed_frame_bytes` の既知の stride×height |
| macOS I420 整合 | issue 0004 で確定した `i420_plane_sizes` の期待値があれば、その**具体値**をテストに固定 |

**含めない（非推奨）**: `i32::MAX` 級の積で `checked_mul` が `None` になるか、という前提のテスト（現状のヘルパでは成立しない）。

## 完了条件（チェックリスト）

- [ ] `cargo test` で上記観点をカバーするテストが追加されている。
- [ ] テスト名・コメントは方針に合わせる（コメントは日本語可）。
