# AVF が存在しない device_id を既定デバイスへ無言フォールバックし、NV12 非対応時にもフォールバックしない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-device-selection
- Polished: {YYYY-MM-DD}

## 目的

`src/video_avf.m` の `video_avf_session_create` に、デバイス・フォーマット選択の問題が 2 つある。

1. `device_id` が指定された場合、`deviceWithUniqueID` が見つからなければ `defaultDeviceWithMediaType` へフォールバックする。存在しない ID を渡した利用者は、別のカメラでキャプチャが始まったことに気づかない。プライバシー・録画対象の取り違えに直結する
2. `pixel_format` が `None` の場合は常に NV12 を要求し、出力が NV12 に対応していなければセッション作成を失敗させる。V4L2 と Windows は YUY2 / I420 へのフォールバックを持つが、AVF だけ持たない
3. `AVCaptureDeviceDiscoverySession` の `deviceTypes` が `AVCaptureDeviceTypeBuiltInWideAngleCamera` と `AVCaptureDeviceTypeExternal` のみで、Continuity Camera（macOS 14.0 以降）や Desk View Camera（macOS 13.0 以降）などが列挙されない。`defaultDeviceWithMediaType` が既定として選ぶデバイスと列挙結果が一致しない可能性がある

## 優先度根拠

- High。(1) は指定と異なるデバイスを掴む誤動作であり、利用者が成功結果だけを受け取る
- (2) は NV12 を出力しないカメラで既定設定のキャプチャが一切できなくなる
- `/review-code` の重要指摘として 3 周にわたり確認済み

## 現状

```objc
if (device_id) {
    NSString* deviceIdStr = [NSString stringWithUTF8String:device_id];
    device = [AVCaptureDevice deviceWithUniqueID:deviceIdStr];
}
if (!device) {
    device = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
}
```

`output_supports_pixel_format` が偽なら `free(videoSession)` して NULL を返し、Rust 側は `Error::SessionCreateFailed` にする。

## 設計方針

- `device_id` 指定時は見つからなければ NULL を返し、Rust 側で `Error::DeviceNotFound` にする。C API の戻り値にエラーコードを追加して区別する
- `pixel_format` が `None` の場合は NV12 → YUY2 → I420 の順で出力対応を確認し、最初に対応する形式を選ぶ
- `device_id` の解決規則を公開 doc に明記する（関連 issue あり）

## 完了条件

- 存在しない `device_id` を指定すると `Error::DeviceNotFound` が返り、別デバイスでキャプチャが始まらない
- NV12 を出力しないカメラでも `pixel_format = None` でキャプチャできる
- 明示指定した `pixel_format` が非対応の場合のエラーが従来どおり返る
- macOS 実機でデバイス指定・フォーマット指定のキャプチャが動作する

## 解決方法

{完了時に記入}
