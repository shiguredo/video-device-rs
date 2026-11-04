# PBT カバレッジの拡充

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/add-pbt-coverage-expansion
- Polished: 2026-11-04

## 目的

プロジェクト規約「PBT でカバーできるものを単体テストで書かないこと」に従い、`frame_math` の純粋関数 7 個に PBT を追加する。

`frame_math` は現在 `pub(crate)` の内部モジュールであり、`pbt/tests/` から直接呼べない。また `pbt` 単体のビルドでは `shiguredo_video_device` を `default-features = false` で依存しているため、`#[cfg(enable_*)]` で条件コンパイルされる関数は `pbt` のビルドに含まれない。本 issue では `frame_math` を公開モジュールにし、全関数を無条件にコンパイルした上で PBT を追加する。

`PixelFormat::Unknown` の PBT 対応は既に完了しているため、本 issue では扱わない。

## 優先度根拠

- Medium。既存コードのバグではないが、プロジェクト規約違反であり、純粋関数の性質検証が不十分
- `frame_math` は整数演算と境界チェックを行う純粋関数であり、PBT の典型対象
- `/review-code` の重要指摘として確認

## 現状

### frame_math に PBT がない

`src/frame_math.rs` の次の 7 関数は、`pub(crate)` の純粋関数として実装されている。

- `nv12_plane_sizes`
- `i420_plane_sizes`
- `yuy2_packed_frame_bytes`
- `nv12_packed_frame_bytes`
- `i420_packed_frame_bytes`
- `yuy2_packed_frame_bytes_win`
- `mjpeg_payload_bytes`

`src/frame_math.rs` の `#[cfg(test)] mod tests` には単体テストがあるが、PBT はない。`pbt/tests/prop_frame_math.rs` も存在しない。

### pbt から frame_math を呼べない

`src/lib.rs` は `mod frame_math;` と宣言しており、モジュールは非公開。各関数も `pub(crate)` のため、別クレートである `pbt` の結合テストからは呼べない。`pbt/tests/prop_types.rs` などの既存 PBT は公開 API だけを対象にしている。

さらに `pbt/Cargo.toml` は `shiguredo_video_device` を `default-features = false` で依存している。`src/frame_math.rs` の関数は `enable_avf` / `enable_v4l2` / `enable_pipewire` / `enable_mf` / `enable_mjpeg` の cfg で条件コンパイルされるため、`cargo test -p pbt` のように `pbt` 単体をビルドする場合は全関数がコンパイル対象外になる。`enable_*` は依存クレートの `build.rs` が設定する cfg であり、`pbt` クレート側のテストでは参照できない。

### PixelFormat::Unknown は対応済み

`pbt/tests/prop_types.rs` の `sample_pixel_format` は noprop のサンプラーを使い、`PixelFormat::Unknown(noprop::sample_u32(ctx))` を含んでいる。`prop_to_raw_matches_expected_fourcc` の `Unknown` アームは `assert_eq!(raw, value, ...)` で生値の保持を検証している。`CHANGES.md` の `## develop` の `### misc` にも noprop 移行と `PixelFormat::Unknown` 追加のエントリがある。したがって本 issue の完了条件には含めない。

`from_raw` は `pub(crate)` であり、PBT からは呼べない。`src/types.rs` の `#[cfg(test)] mod tests` には MJPEG に関する単体テストだけがある。任意の `u32` に対する `from_raw` の検証は本 issue の対象外とする。

## 設計方針

### (1) frame_math を公開 API にする

`pbt` から `frame_math` の全関数を呼べるように、次の変更を行う。

- `src/lib.rs` の `mod frame_math;` を `pub mod frame_math;` に変更する
- `src/frame_math.rs` の 7 関数を `pub(crate) fn` から `pub fn` に変更する
- 各関数の `#[cfg(enable_*)]` を外す。対象は純粋な整数演算だけでプラットフォーム依存がなく、公開 API として全プラットフォームで利用できる
- `#![warn(missing_docs)]` に適合するよう、公開 API としての doc コメントを整える

これにより `pbt` の `default-features = false` ビルドでも全関数がコンパイルされ、`pbt/tests/prop_frame_math.rs` から直接呼べる。

### (2) pbt/tests/prop_frame_math.rs の新規作成

`pbt/tests/prop_error.rs` と `pbt/tests/prop_types.rs` の既存パターンに合わせ、noprop で PBT を実装する。

- `noprop::seed_from_env_or_time(SEED_ENV)` でシードを取得する
- `noprop::Runner::new(seed)` と `runner.run(CASES, |ctx| { ... })` を使う
- 入力は `noprop::sample_i32(ctx)` で生成する
- 期待値は `u128` などの広い型で厳密に計算し、`usize` に収まるかどうかを確認する
- テストのログメッセージは日本語にする

各関数について、次の性質を検証する。

- 任意の `i32` 入力で panic しない
- サイズ入力がすべて正で、`i32` および `usize` の演算がオーバーフローしない場合は `Some` を返す
- `Some` が返る場合、サイズ入力はすべて正である
- `None` が返る場合、少なくとも 1 つのサイズ入力が非正であるか、`usize` への変換または演算がオーバーフローしている
- 固定の小さい値（例: 640x480、奇数 height）でも手計算値と一致する

関数ごとの具体的な性質は次のとおり。

- `nv12_plane_sizes`: `Some((y, uv))` なら `y == stride * height` かつ `uv == stride_uv * div_ceil(height, 2)`
- `i420_plane_sizes`: `Some((y, uv))` なら `y == stride * height` かつ `uv == stride_uv * div_ceil(height, 2) * 2`
- `yuy2_packed_frame_bytes`: `Some(bytes)` なら `bytes == stride * height`
- `nv12_packed_frame_bytes`: `Some(bytes)` なら `bytes == width * height + width * div_ceil(height, 2)`
- `i420_packed_frame_bytes`: `Some(bytes)` なら `bytes == width * height + 2 * div_ceil(width, 2) * div_ceil(height, 2)`
- `yuy2_packed_frame_bytes_win`: `Some(bytes)` なら `bytes == width * 2 * height`。`width > i32::MAX / 2` の場合は `width * 2` が `i32` でオーバーフローするため `None` になる
- `mjpeg_payload_bytes`: `Some(bytes)` なら `bytes == payload_size as usize`

64 bit 環境では `i32` の正値同士の積は `usize` に収まるため、`yuy2_packed_frame_bytes_win` の `width * 2` 以外ではオーバーフローによる `None` は発生しない。32 bit 環境では `usize` の上限を超える場合に `None` となるため、期待値の計算には `u128` を使い、`usize` に収まらない場合は `None` を期待する。

`enable_*` の cfg は `pbt` から参照できないため、PBT 側には付けない。全関数が無条件にコンパイルされる前提で、単一の `pbt/tests/prop_frame_math.rs` にまとめる。

## 完了条件

- `src/lib.rs` で `frame_math` が公開され、`src/frame_math.rs` の 7 関数が `pub fn` になっている
- `src/frame_math.rs` の関数から `#[cfg(enable_*)]` が外れている
- `pbt/tests/prop_frame_math.rs` を新規作成し、7 関数すべての PBT を追加する
- `pbt/tests/prop_types.rs` は変更しない（`PixelFormat::Unknown` は対応済み）
- `cargo test --workspace` と `cargo test -p pbt --tests` が通る
- `CHANGES.md` の `## develop` の `### misc` に `[ADD]` エントリを追加する
  - 例: `[ADD] frame_math のフレームサイズ計算関数を公開し、PBT を追加する`

## 解決方法

{完了時に記入}
