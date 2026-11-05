# ピクセルフォーマットの FourCC 定数を単一の定義源にまとめる

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-fourcc-single-source
- Polished: 2026-11-06

## 目的

`VIDEO_PIXEL_FORMAT_NV12` / `YUY2` / `I420` / `MJPG` の FourCC 値が、`src/video.h` の `#define` と `src/types.rs` の `pub(crate) const` の 2 箇所に定義され、さらに `pbt/tests/prop_types.rs` にも数値が直書きされている。片方だけ変更すると FFI 境界の値がずれてフレームのフォーマット判定が壊れる。定義源を 1 箇所にまとめ、C / Rust / テストが同じ定義源を参照するようにする。

## 優先度根拠

- Low。現状は値が一致しているため直ちに問題は無いが、二重管理は将来の変更で壊れやすい
- `/review-code` の削除・整理候補として確認済み

## 現状

- `src/video.h` には次の `#define` がある。値は ASCII FourCC をリトルエンディアンで表したもので、`0x3231564E` は "NV12"、`0x32595559` は "YUY2"、`0x30323449` は "I420"、`0x47504A4D` は "MJPG" である。

```c
// 値は macOS の CVPixelBuffer が用いる FourCC ('420v' 等) に対応する
#define VIDEO_PIXEL_FORMAT_NV12 0x3231564E  // '420v' (NV12)
#define VIDEO_PIXEL_FORMAT_YUY2 0x32595559  // 'yuvs' (YUY2)
#define VIDEO_PIXEL_FORMAT_I420 0x30323449  // 'I420' (I420)
// MJPEG は圧縮フォーマット。フレームは可変長の JPEG ペイロード。
#define VIDEO_PIXEL_FORMAT_MJPG 0x47504A4D  // 'MJPG' (Motion JPEG, V4L2 互換)
```

- `src/video.h` の先頭コメントと NV12 / YUY2 の行コメントが実際の値と一致していない。`0x3231564E` は "NV12" であり macOS の CVPixelBuffer の '420v'（`0x34323076`）ではない。`0x32595559` は "YUY2" であり 'yuvs'（`0x79757673`）ではない。AVF は CVPixelBuffer の型を `src/video_avf.m` の `convert_pixel_format` で `VIDEO_PIXEL_FORMAT_*` に変換している。
- `src/types.rs` に同じ値の `pub(crate) const` があり、グループ見出しは `/// ピクセルフォーマット定数 (video.h と同じ FourCC 値)` である。
- `pbt/tests/prop_types.rs` の `prop_to_raw_matches_expected_fourcc` に同じ数値のリテラルがある。PBT は別クレートであり、`pub(crate)` の定数を参照できない。
- `build.rs` の `generate_bindings` は allowlist として `allowlist_function("video_.*")` と `allowlist_type` のみを指定し、`allowlist_var` が無いため `#define` マクロは生成されない。生成済みの `bindings_linux.rs` にも `VIDEO_PIXEL_FORMAT_*` は含まれない。
- `issues/0050-bug-avf-uyvy-mislabel.md` で `VIDEO_PIXEL_FORMAT_UYVY` が追加され、`issues/0073-add-uyvy-linux-windows.md` がそれを各バックエンドで使う。先に実施された場合は、本 issue の定義源に UYVY も含める。

## 設計方針

- 定義源は `build.rs` の Rust 定数 1 箇所とする。`src/video.h` の `#define` は削除し、C 側には `cc::Build::define` で同じ値を渡す。
- `build.rs` は `OUT_DIR` に Rust 定数ファイルを生成し、`src/types.rs` から `include!` する。Windows でも build script が Rust 定数ファイルを生成する（Windows では C のビルドは無い）。
- `src/types.rs` の定数群の見出しは `//` にし、定義源が `build.rs` であることを明記する。
- PBT は別クレートで `pub(crate)` 定数を参照できないため、`pbt/tests/prop_types.rs` の FourCC 数値リテラルは削除する。代わりに `src/types.rs` の `#[cfg(test)]` 単体テストで生成された定数を参照し、`PixelFormat::to_raw` が各定数と一致することを全プラットフォームで検証する。`from_raw` のテストは従来どおり FFI バックエンド有効時のみとする。PBT は `Unknown` の生値保持と `name` / `Display` の検証に残す。
- `src/video.h` の先頭コメントを実際の値に合わせる。値が ASCII FourCC の "NV12" / "YUY2" / "I420" / "MJPG" に対応し、macOS の CVPixelBuffer の型（'420v' 等）は AVF の `convert_pixel_format` でこれらの値に変換される旨を書く。`build.rs` の定数には値の根拠をコメントで書く。
- `issues/0050-bug-avf-uyvy-mislabel.md` または `issues/0073-add-uyvy-linux-windows.md` が先に実施されている場合は、`VIDEO_PIXEL_FORMAT_UYVY` も同じ定義源に含める。

## 完了条件

- FourCC の値が `build.rs` の 1 箇所で定義され、C は `cc::Build::define`、Rust は `OUT_DIR` の生成ファイル、テストは `src/types.rs` の単体テストでそれを参照している。
- `src/video.h` の `#define` が削除され、先頭コメントが実際の値（"NV12" / "YUY2" / "I420" / "MJPG"）と一致している。macOS の CVPixelBuffer の型（'420v' 等）は AVF の `convert_pixel_format` でこれらの値に変換される旨が書かれている。
- `pbt/tests/prop_types.rs` の FourCC 数値リテラルが削除され、PBT は `Unknown` の生値保持と `name` / `Display` を検証している。
- `src/types.rs` の定数群の見出しが `//` で、定義源が `build.rs` であることが分かる。
- `VIDEO_PIXEL_FORMAT_UYVY` が追加されている場合は、同じ定義源に含まれている。
- macOS / Linux / Windows の全バックエンドでビルドが通る。
- `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る。

## 解決方法

{完了時に記入}
