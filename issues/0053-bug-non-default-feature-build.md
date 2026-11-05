# 既定バックエンドを無効にした feature 構成で examples と tests がコンパイルできない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-non-default-feature-build
- Polished: {YYYY-MM-DD}

## 目的

README / docs/LINUX.md は「`default-*` で選ばれていないバックエンドでも、feature が有効なら明示 API で利用できる」と説明している。しかし `--no-default-features --features avf` のように既定バックエンドを無効にした構成では、ライブラリ本体はビルドできるが、リポジトリ自身の examples と integration test がコンパイルできない。

再現コマンド（macOS）:

```text
cargo check --no-default-features --features avf --all-targets
```

`examples/device_list.rs` / `examples/device_info.rs` / `examples/camera_preview.rs` / `tests/test_capture.rs` が `VideoDeviceList::enumerate` と `VideoCapture::new` を呼ぶが、これらは `enable_default_*` が定義されるときだけ公開される。examples / tests 側の `#[cfg]` は `any(enable_avf, enable_v4l2, enable_pipewire, enable_mf)` で判定しており、`enable_default_avf` を見ていない。

CI matrix の全構成が `default-*` を含むため、この破綻は一度も検出されない。

## 優先度根拠

- High。README が案内する feature 構成でリポジトリのテストとサンプルがビルドできず、開発・検証ができない
- CI の検証範囲の穴でもある
- `/review-code` の重要指摘として 3 周にわたり確認済み

## 現状

- `src/capture.rs` の `VideoCapture::new` と `src/device.rs` の `VideoDeviceList::enumerate` は `#[cfg(any(enable_default_avf, enable_default_v4l2, enable_default_pipewire, enable_default_mf))]` で公開される
- examples / tests の先頭は `#![cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]` で、既定バックエンドの有無を見ていない
- `cargo check --no-default-features --features avf --all-targets` は exit 101 になる

## 設計方針

- examples / tests の cfg を `any(enable_default_*)` に変更するか、明示 API（`enumerate_avf` / `new_avf` など）を使う版に分岐させる
- CI に `--no-default-features --features <backend>` の check を追加する（関連 issue あり）
- README に「既定 API は `default-*` 有効時のみ」であることを明記する

## 完了条件

- `cargo check --no-default-features --features avf --all-targets`（macOS）、`--features default-v4l2` を外した Linux 構成、`--features default-mf` を外した Windows 構成で、examples / tests を含めてコンパイルできる
- 既定バックエンドありの既存構成が変わらない
- CI で当該構成が検証される

## 解決方法

{完了時に記入}
