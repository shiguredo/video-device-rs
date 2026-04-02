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

- [ ] 選んだ方針 A/B/C が `types.rs` または `video_c.h` に反映されている。
- [ ] 新規のユーザ向けメッセージは英語、コメントは日本語でよい。

## 参考

- `src/capture.rs` の `frame_callback` が `PixelBuffer::from_retained_ptr(pixel_buffer)` を呼ぶ箇所。

## 問題解決

- **非 macOS で非 NULL の `pixel_buffer` を保持するとリークする問題**: 方針 B とし、`from_retained_ptr` は macOS 以外では非 NULL でも `None` を返し、参照を保持しないようにした。
- **契約の明示**: 方針 A に相当する doc を `PixelBuffer`、`VideoFrame` / `VideoFrameOwned`、`video_c.h` に追記し、未対応プラットフォームでは NULL のみとした。
