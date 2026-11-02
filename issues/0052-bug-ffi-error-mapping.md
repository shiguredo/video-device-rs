# FFI バックエンドのエラーが SessionCreateFailed / DeviceAccessDenied に潰れる

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-ffi-error-mapping
- Polished: {YYYY-MM-DD}

## 目的

公開 `Error` には `DeviceNotFound` / `DeviceAccessDenied` / `UnsupportedPixelFormat` / `InvalidCaptureConfig` などの variant があるが、FFI バックエンドでは C 側が失敗を一律 NULL で返すため、ほぼすべてが `SessionCreateFailed` になる。列挙失敗も一律 `DeviceAccessDenied` になる。

具体例:

- AVF のカメラ権限拒否は `SessionCreateFailed` になり、利用者は権限不足だと分からない
- V4L2 の `open` 失敗（デバイス不在）も `SessionCreateFailed`
- AVF の要求フォーマット非対応も `SessionCreateFailed`
- PipeWire デーモン不在時の列挙は `DeviceAccessDenied`（「カメラへのアクセス拒否」）として表示される

`Error` の doc と実挙動が一致していない。

また、`src/capture_ffi.rs` の `FfiCaptureImpl::new` は NUL バイトを含む `device_id` に対して `Error::NullPointer("device_id contains null byte")` を返す。`NullPointer` は「C 側から null ポインタが返された」ことを表す variant であり、設定値の誤りには意味が合わない（`Display` も `null pointer: device_id contains null byte` になる）。

## 優先度根拠

- Medium。動作はするが、利用者が原因を特定して対処できない
- 権限拒否は初回利用時に必ず遭遇する経路であり、診断性の改善価値が高い
- `/review-code` の重要指摘として確認済み

## 現状

`src/capture_ffi.rs` の `FfiCaptureImpl::new` は `session_create` の戻り値が NULL なら一律 `Error::SessionCreateFailed` を返す。`src/device_ffi.rs` の `FfiDeviceListImpl::enumerate` は `ret < 0` を一律 `Error::DeviceAccessDenied` に変換する。

C 側のヘッダは「成功時は 0、失敗時は負の値」としか定義しておらず、失敗理由を区別する契約が無い。

## 設計方針

- C API の戻り値を負のエラーコードに拡張し、Rust 側で対応する `Error` へ変換する
- 最低限、AVF の権限拒否を `DeviceAccessDenied` として区別する
- 列挙失敗とキャプチャ失敗のマッピングをバックエンド間で揃える
- `Error` の各 variant に「どのバックエンドで発生しうるか」を doc として追記する

## 完了条件

- AVF の権限拒否が `Error::DeviceAccessDenied` で返る
- 存在しないデバイス指定が `Error::DeviceNotFound` で返る（バックエンドで可能な範囲）
- PipeWire デーモン不在時に `DeviceAccessDenied` 以外の適切なエラーが返る、または doc で説明される
- `Error` の doc と実挙動が一致する
- 既存の正常系が変わらない

## 解決方法

{完了時に記入}
