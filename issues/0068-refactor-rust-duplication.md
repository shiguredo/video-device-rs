# Rust 側の重複したフレーム構築・列挙ラッパー・文字列取得を統合する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-rust-duplication
- Polished: {YYYY-MM-DD}

## 目的

Rust 側に、ほぼ同一のコードが複数箇所に存在する。片方だけ修正されるリスクを減らすため統合する。

1. `src/capture_ffi.rs` の `frame_callback` の NV12 分岐と I420 分岐は、プレーンサイズ関数と `pixel_format` 以外が完全に同一
2. `src/capture_mf.rs` の `process_sample` の NV12 分岐と I420 分岐も、使用するサイズ関数と `stride_uv` 以外が同一
3. `src/device.rs` の `VideoDeviceList::enumerate_avf` / `enumerate_v4l2` / `enumerate_pipewire` は、呼ぶ列挙関数以外が 8 行ずつ完全に同一
4. `src/capture_mf.rs` の `get_device_symbolic_link` と `src/device_mf.rs` の `get_device_string` は、MF 属性の文字列取得ロジックが重複している
5. `src/capture_ffi.rs` と `src/capture_mf.rs` の内部 impl の doc が公開 API の契約を丸写ししている（doc の重複）

## 優先度根拠

- Low。挙動に影響しない整理
- 重複は将来の修正漏れの温床になる
- `/review-code` の改善指摘として確認済み

## 現状

- `frame_callback` は NV12 / I420 で `VideoFrame { data, uv_data: Some(...), ... }` をそれぞれ構築している
- `device.rs` の 3 つの `enumerate_*` は `inner.as_slice().iter().map(|d| VideoDevice(VideoDeviceInner::Ffi(*d))).collect()` を繰り返す

## 設計方針

- `(y_size, uv_size)` を決める処理だけ分岐し、`VideoFrame` の構築を 1 箇所にまとめる
- `device.rs` に private な `fn from_ffi(inner: FfiDeviceListImpl) -> Self` を用意する
- MF の文字列取得を任意キー版 1 つに統合し、シンボリックリンク取得はキーを渡すだけにする
- 内部 impl の doc は公開 API の契約を参照する 1 行に置き換える

## 完了条件

- 重複が解消されている
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- 全バックエンドのビルドが通る

## 解決方法

{完了時に記入}
