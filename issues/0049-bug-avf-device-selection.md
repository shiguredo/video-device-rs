# AVF が存在しない device_id を既定デバイスへ無言フォールバックし、NV12 非対応時のフォールバックとデバイス列挙が不足している

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-device-selection
- Polished: 2026-11-05

## 目的

`src/video_avf.m` の `video_avf_session_create` と `video_avf_enumerate_devices` に、デバイス・フォーマット選択の問題が 3 つある。

1. `device_id` が指定された場合、`deviceWithUniqueID` が見つからなければ `defaultDeviceWithMediaType` へフォールバックする。存在しない ID を渡した利用者は、別のカメラでキャプチャが始まったことに気づかない。プライバシー・録画対象の取り違えに直結する。C API は失敗理由を区別しないため、Rust 側は `Error::SessionCreateFailed` にするしかない
2. `pixel_format` が `None` の場合は常に NV12 を要求し、出力が NV12 に対応していなければセッション作成を失敗させる。V4L2 は NV12 → YUY2 のフォールバックを持ち、Windows (MF) は NV12 → YUY2 → I420 のフォールバックを持つ。PipeWire も NV12 を優先しつつ YUY2 / I420 を候補にする。AVF だけフォールバックを持たない
3. `AVCaptureDeviceDiscoverySession` の `deviceTypes` に `AVCaptureDeviceTypeContinuityCamera`（macOS 14.0 以降）と `AVCaptureDeviceTypeDeskViewCamera`（macOS 13.0 以降）を指定していないため、これらの種別のデバイスが列挙されない。`defaultDeviceWithMediaType` は `deviceTypes` で絞り込まないため、既定デバイスが列挙結果に現れない可能性がある

## 優先度根拠

- High。(1) は指定と異なるデバイスを掴む誤動作であり、利用者が成功結果だけを受け取る
- (2) は NV12 を出力しないカメラで既定設定のキャプチャが一切できなくなる
- (3) は `VideoDeviceList::enumerate()` に現れないデバイスは `unique_id()` を取得できず、`device_id` に指定できない。Continuity Camera をライブラリ経由で選べない
- `/review-code` の重要指摘として確認済み

## 現状

`src/video_avf.m` の `video_avf_session_create` は次のとおり。

```objc
if (device_id) {
    NSString* deviceIdStr = [NSString stringWithUTF8String:device_id];
    device = [AVCaptureDevice deviceWithUniqueID:deviceIdStr];
}
if (!device) {
    device = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
}
```

`device_id` が NULL でない場合も、`deviceWithUniqueID` が nil を返すと既定デバイスへフォールバックする。`device_id` が NULL の場合と区別できない。

`convert_video_pixel_format_to_cv` は `pixel_format == 0` を NV12 として扱う。`output_supports_pixel_format` が偽なら `free(videoSession)` して NULL を返し、`src/capture_ffi.rs` の `FfiCaptureImpl::new` は NULL を `Error::SessionCreateFailed` にする。NV12 以外を試す経路はない。

`src/video_avf.m` の `video_avf_enumerate_devices` は次のとおり。

```objc
AVCaptureDeviceDiscoverySession* discoverySession = [AVCaptureDeviceDiscoverySession
    discoverySessionWithDeviceTypes:@[AVCaptureDeviceTypeBuiltInWideAngleCamera,
                                       AVCaptureDeviceTypeExternal]
                          mediaType:AVMediaTypeVideo
                           position:AVCaptureDevicePositionUnspecified];
```

`AVCaptureDeviceTypeContinuityCamera` と `AVCaptureDeviceTypeDeskViewCamera` は検索条件に含まれない。

## 設計方針

- `device_id` が NULL でない場合は `deviceWithUniqueID` だけを使い、見つからなければ `defaultDeviceWithMediaType` へフォールバックしない。`device_id` が NULL の場合だけ `defaultDeviceWithMediaType` を使う
- C API にエラーコード出力引数を追加して、`DeviceNotFound` とその他の失敗を区別する。`video_avf_session_create` に `int* error_code` を追加し、`src/video_avf.h` に `0`（成功）、`-1`（その他のセッション作成失敗）、`-2`（`DeviceNotFound`）を定義する。`device_id` が NULL で既定デバイスも無い場合、権限拒否、明示フォーマット非対応などは `-1` とする
- すべての失敗経路で `error_code` を設定する。`src/capture_ffi.rs` の `CaptureOps::session_create` に `error_code` 出力引数を追加し、`FfiCaptureImpl::new` は呼び出し前に `error_code` を `-1` で初期化する。NULL 返却時にのみ値を読み、`-2` を `Error::DeviceNotFound`、その他の負値を `Error::SessionCreateFailed` に変換する。`src/video_v4l2.c` / `src/video_pipewire.c` は同じシグネチャに合わせ、成功時 `0`、失敗時 `-1` を設定する。これは `issues/0052-bug-ffi-error-mapping.md` の C API エラーコード化と同じ方式であり、0052 はこの方式を前提に各バックエンドのエラーコードを拡張する
- `pixel_format` が `None` の場合は `convert_video_pixel_format_to_cv(0)` の NV12 固定を使わず、`output.availableVideoCVPixelFormatTypes` を NV12 → YUY2 → I420 の順に確認し、最初に対応する形式を `output.videoSettings` に設定する。いずれも非対応なら NULL と `-1` を返す
- 明示指定された `pixel_format` は従来どおり変換・対応確認し、非対応なら NULL と `-1` を返す
- `video_avf_enumerate_devices` の `deviceTypes` に `AVCaptureDeviceTypeContinuityCamera` と `AVCaptureDeviceTypeDeskViewCamera` を追加する。README のとおり macOS 14.0 以上を前提とするため、`@available` は不要
- `AVCaptureDeviceTypeDeskViewCamera` は `AVCaptureMultiCamSession` 向けの仮想カメラである。本 issue では列挙対象に含めるが、通常の `AVCaptureSession` でキャプチャできるかは別途確認し、できない場合は `SessionCreateFailed` と doc に明記する
- `skills/shiguredo-video-device/SKILL.md` のバックエンド差表、エラー表、macOS の列挙対象と既知の制限事項を、本 issue の新しい挙動に合わせて更新する。公開 API の rustdoc のバックエンド差は `issues/0062-doc-public-api-backend-differences.md` で扱う

## 変更対象

- `src/video_avf.m`: `video_avf_session_create` の `device_id` 解決、`pixel_format` フォールバック、`video_avf_enumerate_devices` の `deviceTypes`
- `src/video_avf.h`: `video_avf_session_create` のエラーコード出力引数とエラーコード定義
- `src/capture_ffi.rs`: `CaptureOps::session_create` のシグネチャと `FfiCaptureImpl::new` のエラー変換
- `src/video_v4l2.c` / `src/video_v4l2.h`: `video_v4l2_session_create` のシグネチャを合わせ、成功時 0 / 失敗時 -1 を設定
- `src/video_pipewire.c` / `src/video_pipewire.h`: `video_pipewire_session_create` のシグネチャを合わせ、成功時 0 / 失敗時 -1 を設定
- `skills/shiguredo-video-device/SKILL.md`: バックエンド差表、エラー表、macOS の列挙対象と既知の制限事項
- `CHANGES.md`: `## develop` に `[FIX]` エントリを追加

## 完了条件

- `device_id` が NULL でなく、一致するデバイスが無い場合に `Error::DeviceNotFound` が返り、既定デバイスでキャプチャが始まらない
- `device_id` が NULL の場合は既定デバイスを使い、既定デバイスも無い場合は `Error::SessionCreateFailed` が返る
- `pixel_format = None` で、NV12 を出力しないが YUY2 または I420 を出力できるカメラからキャプチャでき、コールバックの `VideoFrame::pixel_format` が選択された形式になる
- NV12 / YUY2 / I420 のいずれも出力できないカメラでは、従来どおり `Error::SessionCreateFailed` が返る
- 明示指定した `pixel_format` が出力非対応の場合は、従来どおりのエラーが返る（AVF の C API に渡る NV12 / YUY2 / I420 は `Error::SessionCreateFailed`、`PixelFormat::Mjpeg` は `Error::UnsupportedPixelFormat`）
- 利用可能な実機環境で `VideoDeviceList::enumerate()` に Continuity Camera と Desk View Camera が含まれ、`unique_id()` を `device_id` に渡せる
- `skills/shiguredo-video-device/SKILL.md` のバックエンド差表、エラー表、macOS の列挙対象と既知の制限事項が実装と一致する
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する
- macOS 実機で、存在しない `device_id` の `DeviceNotFound`、`pixel_format = None` のフォールバック、Continuity Camera / Desk View Camera の列挙を確認する。CI の macOS では TCC のためキャプチャテストがスキップされるので、キャプチャは手元の実機で確認する
- `cargo build --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通る

## スコープ外

- AVF の権限拒否や明示フォーマット非対応など、`DeviceNotFound` 以外のエラー種別の細分化は `issues/0052-bug-ffi-error-mapping.md` で扱う。本 issue では C API の他の失敗を `Error::SessionCreateFailed` のままにする（`PixelFormat::Mjpeg` は `new_avf` が `Error::UnsupportedPixelFormat` を返す既存挙動を維持する）
- 空文字列の `device_id` は、空の `unique_id` を持つデバイスが無ければ `deviceWithUniqueID:` が nil を返すため `Error::DeviceNotFound` になる。`issues/0035-bug-avf-error-path-hardening.md` の「空の `unique_id` はデフォルトデバイスへフォールバックする」という記述は、0035 の実装時に本 issue の挙動へ合わせて更新する
- UYVY（'2vuy'）の扱いは `issues/0050-bug-avf-uyvy-mislabel.md` で扱う。本 issue のフォールバックは `PixelFormat::Yuy2`（'yuvs'）を対象とし、0050 で Uyvy variant が追加された場合のフォールバック追加は本 issue では扱わない
- Desk View Camera を通常の `AVCaptureSession` でキャプチャするための `AVCaptureMultiCamSession` への切り替えは本 issue では扱わない。列挙のみ対象
- V4L2 / MF のフォールバック順は変更しない
- 公開 API の rustdoc のバックエンド差は `issues/0062-doc-public-api-backend-differences.md` で扱う

## 解決方法

{完了時に記入}
