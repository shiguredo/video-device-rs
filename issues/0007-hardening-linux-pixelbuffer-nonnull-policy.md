# 非 macOS で `pixel_buffer` が非 NULL のときの扱いを固定する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

**メモリリーク**の防止。`PixelBuffer` の `Drop` は **`#[cfg(target_os = "macos")]` のみ `CFRelease`**（`src/types.rs` 約 112〜120 行）。Linux では `CFRelease` しないため、`from_retained_ptr` が非 NULL を受け取ると参照が解放されず**リーク**しうる。

## 契約

- `src/video_c.h` 31 行付近: 未対応プラットフォームでは `pixel_buffer` は **NULL**。

## 修正方針（いずれかを選び、issue 内で一貫させる）

**A. 文書のみ（最小）**

- `PixelBuffer` と `VideoFrame::pixel_buffer` の doc に、「macOS 以外では C から常に NULL が渡る。非 NULL はサポート外」と**日本語で明記**。
- `video_c.h` の `pixel_buffer` 説明を 1 行補強（英語）。

**B. 実装で堅牢化（推奨）**

- `PixelBuffer::from_retained_ptr`（`types.rs` 約 89〜96 行）を、`cfg(not(target_os = "macos"))` では **`ptr.is_null()` でなければ `None` を返す**（非 NULL を無視し、リークしない）。  
  または非 NULL のとき `debug_assert!` / ログ（ログメッセージは英語）のみ。

**C. V4L2 / PipeWire の C が確実に NULL を渡している**ことを C 側コメントで再確認し、Rust は A のみ。

## 完了条件（チェックリスト）

- [x] 選んだ方針 A/B/C が `types.rs` または `video_c.h` に反映されている。
- [x] 新規のユーザ向けメッセージは英語、コメントは日本語でよい。

## 完了条件の検証

2026-04-02 に `types.rs` と `video_c.h` を確認した。

- 方針 B（非 NULL を保持しない）と方針 A に相当する doc 追記: `from_retained_ptr` は非 macOS で非 NULL でも `None`（`types.rs` 約 91〜103 行）。`PixelBuffer` / `VideoFrame` / `VideoFrameOwned` と `video_c.h` の `pixel_buffer` を更新。
- 利用者向けの `Display` 等は既存どおり英語。コメントは日本語。

## 参考

- `src/capture.rs` の `frame_callback` が `PixelBuffer::from_retained_ptr(pixel_buffer)` を呼ぶ箇所。

## 問題解決

### 問題だったこと

- macOS 以外では `PixelBuffer::Drop` が `CFRelease` しないため、**`from_retained_ptr` が非 NULL を `Some` で保持すると参照が解放されずリーク**しうる。一方 `video_c.h` では未対応プラットフォームは `pixel_buffer` を NULL とする契約である。

### どう解決したか

- **方針 B**: `cfg(not(target_os = "macos"))` では、ポインタが非 NULL でも **`None` を返し**オペーク参照を保持しない（リークしない）。
- **方針 A に相当する文書化**: `PixelBuffer`、`VideoFrame` / `VideoFrameOwned` の `pixel_buffer`、`video_c.h` の `pixel_buffer` 説明を、macOS 以外では NULL のみ・非 NULL は未サポートと明記した。

### issue 本文との差異

- **方針 C**（V4L2/PipeWire の C 側で NULL を再確認するコメントのみ、Rust は A のみ）は**採用していない**。B と A を併用した。
- 方針 B の代替として書かれていた「`debug_assert!` やログのみ」は使わず、**非 NULL は保持しない**実装にした。
