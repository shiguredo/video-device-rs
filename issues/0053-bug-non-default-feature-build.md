# 既定バックエンドを無効にした feature 構成で examples と tests がコンパイルできない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-non-default-feature-build
- Polished: 2026-11-05

## 目的

README / docs/LINUX.md は「`default-*` で選ばれていないバックエンドでも、feature が有効なら明示 API で利用できる」と説明している。しかし `--no-default-features --features avf` のように既定バックエンドを無効にした構成では、ライブラリ本体はビルドできるが、リポジトリ自身の examples と integration test がコンパイルできない。

再現コマンド（macOS）:

```text
cargo check --no-default-features --features avf --all-targets
```

`examples/device_list.rs` と `examples/device_info.rs` は `VideoDeviceList::enumerate()` を、`examples/camera_preview.rs` と `tests/test_capture.rs` は `VideoDeviceList::enumerate()` と `VideoCapture::new()` を呼ぶが、これらは `enable_default_*` が定義されるときだけ公開される。examples / tests 側の cfg は `any(enable_avf, enable_v4l2, enable_pipewire, enable_mf)` で判定しており、`enable_default_*` を見ていない。

CI matrix の全構成が `default-*` を含むため、この破綻は一度も検出されない。

## 優先度根拠

- High。README が案内する feature 構成でリポジトリのテストとサンプルがビルドできず、開発・検証ができない
- CI の検証範囲の穴でもある
- `/review-code` の重要指摘として 3 周にわたり確認済み

## 現状

- `src/capture.rs` の `VideoCapture::new` と `src/device.rs` の `VideoDeviceList::enumerate` は `#[cfg(any(enable_default_avf, enable_default_v4l2, enable_default_pipewire, enable_default_mf))]` で公開される
- `tests/test_capture.rs` の先頭は `#![cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]`、examples は `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]` で判定しており、`enable_default_*` を見ていない
- macOS で `cargo check --no-default-features --features avf --all-targets` は exit 101 になる。Linux でも `cargo check --no-default-features --features v4l2 --all-targets` が exit 101 になり、`examples/device_list.rs` / `examples/device_info.rs` / `examples/camera_preview.rs` / `tests/test_capture.rs` の 4 ファイルで `E0599` になることを実測で確認済み
- README の「`default-*` を 1 つも有効にしていないプラットフォームでは、`VideoCapture::new()` と `VideoDeviceList::enumerate()` は存在しません。明示 API を使ってください。」は既に記載済みで、変更は不要

## 設計方針

- examples / tests は、`enable_default_*` が有効な場合は既定 API（`VideoDeviceList::enumerate()` / `VideoCapture::new()`）を、無効な場合は有効なバックエンドの明示 API（`enumerate_avf()` / `new_avf()` など）を呼ぶように変更する。`any(enable_default_*)` で examples / tests 全体をコンパイル対象から外す方式は採用しない（明示 API の動作をサンプルとテストで確認できなくなるため）
- 明示 API を選ぶ場合の優先順は `avf` → `v4l2` → `pipewire` → `mf` とし、最初に有効なバックエンドを使う。Linux で `v4l2` と `pipewire` が同時に有効かつ `default-*` が無い場合は V4L2 を優先する（README / docs/LINUX.md の既定と同じ）
- 変更対象は `examples/device_list.rs`、`examples/device_info.rs`、`examples/camera_preview.rs`、`tests/test_capture.rs`
- CI への `--no-default-features --features <backend>` の check 追加は `issues/0058-update-ci-release-hardening.md` の「`--no-default-features` 構成が CI に無い」で扱う。本 issue では CI を変更しない。0058 の CI 追加では、`--no-default-features --features v4l2` など、既定バックエンド無しでバックエンド feature だけを有効にした構成を含める

## 完了条件

- macOS: `cargo check --no-default-features --features avf --all-targets` が通る
- Linux: `cargo check --no-default-features --features v4l2 --all-targets`、`cargo check --no-default-features --features pipewire --all-targets`、`cargo check --no-default-features --features v4l2,pipewire --all-targets`、`cargo check --no-default-features --features mjpeg --all-targets` が通る
- Windows: `cargo check --no-default-features --features mf --all-targets` が通る
- `cargo check --no-default-features --all-targets`（バックエンド無し）が引き続き通る
- 既定バックエンドありの既存構成（CI matrix の全構成）が変わらない。`cargo check --all-targets` と `cargo test --workspace` が通る
- `cargo fmt --all --check` と `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
