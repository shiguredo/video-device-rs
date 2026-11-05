# Rust 側の重複したフレーム構築・列挙ラッパー・文字列取得と doc を整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-rust-duplication
- Polished: 2026-11-06

## 目的

Rust 側に、ほぼ同一のコードや doc が複数箇所に存在する。片方だけ修正されるリスクを減らすため整理する。

1. `src/capture_ffi.rs` の `frame_callback` の NV12 分岐と I420 分岐の本体は、呼ぶプレーンサイズ関数（`nv12_plane_sizes` / `i420_plane_sizes`）とコメント以外が完全に同一
2. `src/capture_mf.rs` の `process_sample` の NV12 分岐と I420 分岐の本体も、使用するサイズ関数と `stride_uv` 以外が同一
3. `src/device.rs` の `VideoDeviceList::enumerate_avf` / `enumerate_v4l2` / `enumerate_pipewire` は、関数本体のうち、呼ぶ列挙関数以外の 9 行が完全に同一
4. `src/capture_mf.rs` の `get_device_symbolic_link` と `src/device_mf.rs` の `get_device_string` は、MF 属性の文字列取得ロジックが重複している
5. `src/capture_ffi.rs` と `src/capture_mf.rs` の内部 impl の doc が公開 API の doc の大部分を複製している（doc の重複）

## 優先度根拠

- Low。挙動に影響しない整理
- 重複は将来の修正漏れの温床になる
- `/review-code` の改善指摘として起票され、本審でも実ファイルで重複を再確認した

## 現状

- `frame_callback` は NV12 / I420 で `VideoFrame { data, uv_data: Some(...), ... }` をそれぞれ構築している
- `process_sample` も NV12 / I420 で `VideoFrame` をそれぞれ構築している
- `device.rs` の 3 つの `enumerate_*` は `inner.as_slice().iter().map(|d| VideoDevice(VideoDeviceInner::Ffi(*d))).collect()` を繰り返す
- `get_device_symbolic_link` は `get_device_string` と同じ MF 属性取得処理を、シンボリックリンクのキー固定で実装している
- `FfiCaptureImpl` / `MfCaptureImpl` の `start` / `stop` / `config` などの doc は、公開 API の doc を複製している

## 設計方針

- `frame_callback` の NV12 / I420 は、`uv_data` の NULL チェックと `(y_size, uv_size)` の計算だけを分岐し、`VideoFrame` の構築を 1 箇所にまとめる
- `process_sample` の NV12 / I420 は、`required` と `stride_uv` の計算だけを分岐し、`y_size` の計算と `VideoFrame` の構築を 1 箇所にまとめる
- YUY2 / MJPEG の分岐は本 issue の対象外とし、現状のまま残す
- `device.rs` に private な `fn from_ffi(inner: FfiDeviceListImpl) -> Self` を用意する。`FfiDeviceListImpl` は FFI バックエンド有効時だけ存在するため、`#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` を付ける
- MF の文字列取得を `src/device_mf.rs` の `get_device_string` に統合し、`pub(crate)` にする
- `src/capture_mf.rs` の `get_device_symbolic_link` は削除し、`get_device_string(activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)` を直接呼ぶ。新しいモジュールは作らない
- 内部 impl の doc は、公開 API の該当メソッド（`VideoCapture::start` / `stop` / `config` など）を参照する 1 行に置き換える。公開 API にない内部固有の補足だけは必要に応じて残す。例: ``/// 公開 API [`VideoCapture::start`](crate::VideoCapture::start) の契約に従う。``

## 完了条件

- 既存の公開 API の挙動が変わっていないこと
- 目的に列挙した 5 項目の重複が解消されている
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- CI の全バックエンド matrix（Ubuntu の V4L2 / PipeWire / 両方、Ubuntu ARM、macOS、Windows）でビルドとテストが通る

## 解決方法

{完了時に記入}
