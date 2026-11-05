# FFI バックエンドのエラーが SessionCreateFailed / DeviceAccessDenied に潰れる

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-ffi-error-mapping
- Polished: 2026-11-05

## 目的

公開 `Error` には `DeviceNotFound` / `DeviceAccessDenied` / `UnsupportedPixelFormat` / `InvalidCaptureConfig` などの variant があるが、FFI バックエンド (AVF / V4L2 / PipeWire) では C 側が失敗理由を区別せず、次のように原因を特定できないエラーになる。

- AVF のカメラ権限拒否は `SessionCreateFailed` になり、利用者は権限不足だと分からない
- V4L2 の `open` 失敗 (デバイス不在) も `SessionCreateFailed`
- AVF / V4L2 の明示指定したピクセルフォーマット非対応も `SessionCreateFailed`
- PipeWire デーモン不在時の列挙は `DeviceAccessDenied` (「カメラへのアクセス拒否」) として表示される
- 列挙失敗はすべて `DeviceAccessDenied` になる

`src/error.rs` の `DeviceAccessDenied` と `NullPointer` の doc、`README.md` のカメラ権限の記述、`skills/shiguredo-video-device/SKILL.md` のエラー表と既知の制限事項が実挙動と一致していない。

また、`src/capture_ffi.rs` の `FfiCaptureImpl::new` は NUL バイトを含む `device_id` に対して `Error::NullPointer("device_id contains null byte")` を返す。`NullPointer` は「C 側から null ポインタが返された」ことを表す variant であり、設定値の誤りには意味が合わない (`Display` も `null pointer: device_id contains null byte` になる)。

## 優先度根拠

- Medium。動作はするが、利用者が原因を特定して対処できない
- 権限拒否は初回利用時に遭遇しうる経路であり、診断性の改善価値が高い
- `/review-code` の重要指摘として確認済み

## 現状

`src/capture_ffi.rs` の `FfiCaptureImpl::new` は `CaptureOps::session_create` の戻り値が NULL なら一律 `Error::SessionCreateFailed` を返す。`src/device_ffi.rs` の `FfiDeviceListImpl::enumerate` は `DeviceOps::enumerate_devices` の戻り値が負なら一律 `Error::DeviceAccessDenied` に変換する。

C 側の失敗経路は次のとおり。

- `src/video_avf.m` の `video_avf_session_create` は、権限拒否 (`AVAuthorizationStatusDenied` / `AVAuthorizationStatusRestricted`)、明示指定フォーマットの非対応 (`output_supports_pixel_format`)、デバイス入力・出力の追加失敗などをすべて NULL で返す
- `src/video_v4l2.c` の `video_v4l2_session_create` は、`open` の失敗 (デバイス不在・権限不足・その他)、`VIDIOC_QUERYCAP` / `VIDIOC_S_FMT` / `init_mmap` の失敗をすべて NULL で返す
- `src/video_pipewire.c` の `video_pipewire_session_create` は、確保失敗、`pw_thread_loop_new` / `pw_context_new` の失敗、明示指定フォーマットの変換失敗をすべて NULL で返す。デーモンへの接続は `session_create` では行わない
- `src/video_avf.m` / `src/video_v4l2.c` / `src/video_pipewire.c` の `enumerate_devices` は、引数不正・確保失敗・PipeWire の初期化・接続・レジストリ取得失敗を負の値で返すが、値の意味はバックエンドごとに異なり、Rust 側はすべて `DeviceAccessDenied` に潰している

`src/video_avf.h` / `src/video_v4l2.h` / `src/video_pipewire.h` は、列挙と `session_start` について「成功時は 0、失敗時は負の値」としか定義しておらず、失敗理由を区別する契約が無い。`session_create` は `struct VideoSession*` を返すため、負のエラーコードを返す余地自体が無い。

## 設計方針

C API に失敗理由を表すエラーコードを導入し、Rust 側で対応する `Error` へ変換する。`session_create` はポインタを返すため、`int* error_code` 出力引数を追加する。これは `issues/0049-bug-avf-device-selection.md` で AVF の `DeviceNotFound` のために導入される方式と同じであり、0049 が先に実装されている場合はその引数と `0` / `-1` / `-2` の割り当てを維持したうえで、本 issue が `-3` 以降を追加する。0049 が未実装の場合は本 issue で同じ出力引数を導入する。

`src/video.h` に共通のエラーコード定数を定義する。0049 が `src/video_avf.h` に定義済みの場合は `src/video.h` へ移す。

- `VIDEO_ERROR_NONE` = 0: 成功
- `VIDEO_ERROR_GENERIC` = -1: その他の失敗
- `VIDEO_ERROR_DEVICE_NOT_FOUND` = -2: 明示指定したデバイスが存在しない
- `VIDEO_ERROR_ACCESS_DENIED` = -3: 権限拒否
- `VIDEO_ERROR_UNSUPPORTED_PIXEL_FORMAT` = -4: 明示指定したピクセルフォーマットが非対応
- `VIDEO_ERROR_BACKEND_UNAVAILABLE` = -5: バックエンドの初期化・接続・列挙に失敗

Rust 側は `src/types.rs` の `VIDEO_PIXEL_FORMAT_*` と同様に、`src/video.h` と同じ値を private 定数として持つ (bindgen の生成物には依存しない)。

`video_*_session_create` のシグネチャに `int* error_code` を追加し、成功時は `VIDEO_ERROR_NONE`、失敗時は対応するコードを設定する。`video_*_enumerate_devices` は戻り値で同じコードを返す。C 側はすべての失敗経路で `error_code` を設定する。Rust 側は呼び出し前に `VIDEO_ERROR_GENERIC` で初期化し、C 側の設定漏れがあっても `SessionCreateFailed` に落ちるようにする。C 側は `error_code` が NULL でも安全に動作すること (Rust 側は常に非 NULL を渡す)。

Rust 側の `CaptureOps::session_create` と `DeviceOps::enumerate_devices` の型を更新し、`FfiCaptureImpl::new` と `FfiDeviceListImpl::enumerate` で次のように変換する。

| C のエラーコード | `FfiCaptureImpl::new` の変換 | `FfiDeviceListImpl::enumerate` の変換 |
| --- | --- | --- |
| `VIDEO_ERROR_DEVICE_NOT_FOUND` | `Error::DeviceNotFound` | `Error::DeviceNotFound` |
| `VIDEO_ERROR_ACCESS_DENIED` | `Error::DeviceAccessDenied` | `Error::DeviceAccessDenied` |
| `VIDEO_ERROR_UNSUPPORTED_PIXEL_FORMAT` | `Error::UnsupportedPixelFormat(指定されたフォーマット)` | `Error::BackendUnavailable` (列挙では使わない) |
| `VIDEO_ERROR_BACKEND_UNAVAILABLE` | `Error::BackendUnavailable` | `Error::BackendUnavailable` |
| `VIDEO_ERROR_GENERIC` およびその他の負値 | `Error::SessionCreateFailed` | `Error::BackendUnavailable` |

`session_create` が NULL を返したときに `error_code` が `VIDEO_ERROR_NONE` (0) の場合は、C 側の設定漏れとして `SessionCreateFailed` にする。列挙では 0 は成功なので、表は負値の場合にだけ適用する。

`VIDEO_ERROR_UNSUPPORTED_PIXEL_FORMAT` は、`config.pixel_format` が `Some` のときだけ `UnsupportedPixelFormat` に変換する。`None` の場合は `SessionCreateFailed` にする (利用者が特定のフォーマットを要求していないため)。

各バックエンドのコード割り当ては次のとおり。

- AVF: 権限拒否は `-3`、0049 実装後の明示デバイス不在は `-2`、明示指定フォーマットの非対応は `-4`、それ以外 (既定デバイス不在、入力・出力の追加失敗、確保失敗など) は `-1`
- V4L2: `open` の `ENOENT` かつ `device_id` が非 NULL の場合は `-2`、`open` の `EACCES` は `-3`、明示指定フォーマットの変換失敗または `VIDIOC_S_FMT` の不一致は `-4`、それ以外は `-1`。`device_id` が NULL の既定 `/dev/video0` の `ENOENT` は `-1`
- PipeWire: 明示指定フォーマットの変換失敗は `-4`、`pw_thread_loop_new` / `pw_context_new` の失敗は `-5`、それ以外は `-1`
- 列挙: AVF / V4L2 の確保失敗と PipeWire の初期化・接続・レジストリ取得失敗は `-5`、引数不正は `-1`。列挙では `-2` / `-4` は使わない

`Error` に `BackendUnavailable` を追加する。`Display` は `"backend unavailable"`、doc は「バックエンドの初期化・接続・列挙に失敗した」とする。`Error` は `#[non_exhaustive]` ではないため、variant の追加は後方互換のない変更であり、`CHANGES.md` に `[CHANGE]` を追加する。

`FfiCaptureImpl::new` の NUL バイトを含む `device_id` は `Error::NullPointer` ではなく `Error::InvalidCaptureConfig("device_id contains null byte")` にする。`NullPointer` は C 側が NULL を返した場合 (`device name` / `device unique_id`) に限定する。

Rust 側の変換ロジックは private 関数 (`map_session_create_error` / `map_enumerate_error` など) に切り出し、C のエラーコードごとの変換を単体テストで検証する。

本 issue の対象は FFI バックエンドの `session_create` と `enumerate_devices` とする。`session_start` の失敗は `Error::SessionStartFailed` のまま、Windows (MF) の `src/device_mf.rs` の列挙エラーは `DeviceAccessDenied` のままとし、バックエンド間の統一は FFI 内に限定する。

`src/error.rs` の doc、`README.md` のカメラ権限と PipeWire の記述、`skills/shiguredo-video-device/SKILL.md` のエラー表・バックエンド差表・既知の制限事項を、上記のマッピングに合わせて更新する。全 variant のバックエンド差の網羅的な整理は `issues/0062-doc-public-api-backend-differences.md` で扱うため、本 issue では挙動が変わる variant の記述に限定する。

## 変更対象

- `src/video.h`: 共通のエラーコード定数
- `src/video_avf.h` / `src/video_avf.m`: `video_avf_session_create` の `error_code` 出力引数、各失敗経路のコード設定、`video_avf_enumerate_devices` のコード
- `src/video_v4l2.h` / `src/video_v4l2.c`: `video_v4l2_session_create` の `error_code` 出力引数、各失敗経路のコード設定、`video_v4l2_enumerate_devices` のコード
- `src/video_pipewire.h` / `src/video_pipewire.c`: `video_pipewire_session_create` の `error_code` 出力引数、各失敗経路のコード設定、`video_pipewire_enumerate_devices` のコード
- `src/capture_ffi.rs`: `CaptureOps::session_create` のシグネチャ、`FfiCaptureImpl::new` の変換、NUL バイトの `InvalidCaptureConfig`、doc
- `src/device_ffi.rs`: `DeviceOps::enumerate_devices` の変換、doc
- `src/error.rs`: `BackendUnavailable` の追加、`Display`、`DeviceAccessDenied` / `DeviceNotFound` / `SessionCreateFailed` / `UnsupportedPixelFormat` / `NullPointer` / `InvalidCaptureConfig` の doc
- `README.md`: macOS のカメラ権限のエラー、PipeWire の列挙エラー
- `skills/shiguredo-video-device/SKILL.md`: エラー表、バックエンド差表、既知の制限事項、macOS の権限の記述
- `tests/test_capture.rs`: NUL バイトと V4L2 の不在デバイスのテスト (実機不要) と、実機不要のテストを含む旨へのモジュール doc の更新
- `pbt/tests/prop_error.rs`: `BackendUnavailable` の追加
- `CHANGES.md`: `[FIX]` と `[CHANGE]`

## 完了条件

- AVF の権限拒否が `Error::DeviceAccessDenied` で返る
- V4L2 の明示指定した存在しないデバイス (`open` の `ENOENT`) が `Error::DeviceNotFound` で返る
- V4L2 の `open` の `EACCES` が `Error::DeviceAccessDenied` で返る
- AVF / V4L2 の明示指定したピクセルフォーマット非対応が `Error::UnsupportedPixelFormat` で返る
- PipeWire デーモン不在時の `VideoDeviceList::enumerate_pipewire()` が `Error::BackendUnavailable` で返る
- NUL バイトを含む `device_id` が `Error::InvalidCaptureConfig("device_id contains null byte")` で返る
- `Error` の doc、`README.md`、`skills/shiguredo-video-device/SKILL.md` の記述が実装と一致する
- 既存の正常系が変わらない
- C のエラーコードごとの Rust 側の変換を検証する単体テストが実機不要で実行できる
- `cargo build --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通る
- Linux で `cargo build --workspace --features v4l2,pipewire`、`cargo clippy --workspace --all-targets --features v4l2,pipewire -- -D warnings`、`cargo test --workspace --features v4l2,pipewire` が通る
- Linux で `cargo build --workspace --no-default-features --features default-pipewire` が通る
- `CHANGES.md` の `## develop` に `[FIX]` と `[CHANGE]` が追加されている

## 確認方法

- NUL バイトを含む `device_id` と V4L2 の不在デバイスは、実機不要のテストで確認する
- AVF の権限拒否は macOS 実機で、カメラ権限を拒否した状態で `VideoCapture::new_avf()` が `Error::DeviceAccessDenied` を返すことを確認する
- PipeWire デーモン不在は、デーモンを停止した状態で `VideoDeviceList::enumerate_pipewire()` が `Error::BackendUnavailable` を返すことを確認する
- AVF / V4L2 の明示指定フォーマット非対応は、非対応フォーマットを指定して `Error::UnsupportedPixelFormat` を返すことを確認する。AVF は実機、V4L2 は実機または非対応フォーマットを返すデバイスで確認する

## スコープ外

- AVF の明示デバイス不在 (`-2`) は `issues/0049-bug-avf-device-selection.md` の `device_id` 解決修正で発生する。本 issue は `-2` を `Error::DeviceNotFound` に変換する経路を用意するが、0049 が未実装の間は AVF は明示デバイス不在を検出できず、既定デバイスへフォールバックするか `SessionCreateFailed` になる
- PipeWire は `session_create` でデーモンに接続しないため、存在しない `device_id` は `new_pipewire()` では検出できない。`start()` の `SessionStartFailed` になるか、セッションマネージャーの挙動に依存する (本 issue の対象外)
- `session_start` の失敗は `Error::SessionStartFailed` のままとし、本 issue では変更しない
- Windows (MF) の `src/device_mf.rs` の列挙エラーは `DeviceAccessDenied` のままとし、本 issue では変更しない
- `issues/0049-bug-avf-device-selection.md` の完了条件にある「AVF の明示フォーマット非対応は `SessionCreateFailed`」は、本 issue 実装前の挙動を指す。本 issue 実装後は `Error::UnsupportedPixelFormat` になる
- `issues/0050-bug-avf-uyvy-mislabel.md` の「FFI は `SessionCreateFailed`」という記述も、本 issue 実装前の挙動を指す。本 issue 実装後、明示指定した非対応フォーマットは `Error::UnsupportedPixelFormat` になる
- 全 `Error` variant のバックエンド差の網羅的な doc 整理は `issues/0062-doc-public-api-backend-differences.md` で扱う

## 解決方法

{完了時に記入}
