# ピクセルフォーマットの FourCC 定数を単一の定義源にまとめる

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-fourcc-single-source
- Polished: {YYYY-MM-DD}

## 目的

`VIDEO_PIXEL_FORMAT_NV12` / `YUY2` / `I420` / `MJPG` の FourCC 値が、`src/video.h` の `#define` と `src/types.rs` の `pub(crate) const` の 2 箇所に定義され、さらに `pbt/tests/prop_types.rs` にも数値が直書きされている。片方だけ変更すると FFI 境界の値がずれてフレームのフォーマット判定が壊れる。

## 優先度根拠

- Low。現状は値が一致しているため直ちに問題は無いが、二重管理は将来の変更で壊れやすい
- `/review-code` の削除・整理候補として確認済み

## 現状

- `src/video.h`:

```c
#define VIDEO_PIXEL_FORMAT_NV12 0x3231564E
#define VIDEO_PIXEL_FORMAT_YUY2 0x32595559
#define VIDEO_PIXEL_FORMAT_I420 0x30323449
#define VIDEO_PIXEL_FORMAT_MJPG 0x47504A4D
```

- `src/types.rs` に同じ値の `pub(crate) const`
- `pbt/tests/prop_types.rs` に同じ数値のリテラル

`build.rs` の bindgen は `allowlist_function("video_.*")` のみで、マクロを取り込んでいない。

## 設計方針

- 定義源を 1 箇所にする。候補は次のいずれか。
  - `src/video.h` の定数を C の `enum` など bindgen が取り込める形にし、Windows（bindgen を呼ばない）向けには build script で同じ値の Rust 定数を生成する
  - build script を定義源にし、C 側へは `cc::Build::define` で渡し、Rust 側へは生成ファイルで渡す
- PBT のリテラルは定義源の定数を参照する

## 完了条件

- FourCC の値が 1 箇所で定義され、C と Rust とテストがそれを参照している
- macOS / Linux / Windows の全バックエンドでビルドが通る
- `cargo test --workspace` と PBT が通る
- `src/video.h` の FourCC コメントが実際の値（"NV12" / "YUY2" / "I420" / "MJPG"）と一致する

## 解決方法

{完了時に記入}
