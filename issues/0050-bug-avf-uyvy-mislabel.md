# AVF が UYVY（'2vuy'）を YUY2 として扱う

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-uyvy-mislabel
- Polished: {YYYY-MM-DD}

## 目的

`src/video_avf.m` の `convert_pixel_format` と `captureOutput` が、`kCVPixelFormatType_422YpCbCr8`（FourCC '2vuy' = UYVY、Cb Y'0 Cr Y'1 の並び）を `PixelFormat::Yuy2` として扱っている。`kCVPixelFormatType_422YpCbCr8_yuvs`（'yuvs' = YUY2、Y'0 Cb Y'1 Cr の並び）とはバイト順が異なる別フォーマットである。

結果として次の問題がある。

- `VideoDevice::formats()` が '2vuy' のみに対応するカメラを「YUY2 対応」と報告する。しかしキャプチャでは 'yuvs' を要求するため、そのカメラは列挙されるのにキャプチャできない
- 何らかの経路で '2vuy' のフレームが届いた場合、Cb と Cr が入れ替わった色で利用者へ渡る

## 優先度根拠

- Medium。列挙結果の誤りと色データの誤認につながる
- 現行のキャプチャ経路は 'yuvs' しか要求しないため、フレームの色化けは通常発生しないが、外部カメラで '2vuy' のみを提示する機種は存在する
- `/review-code` の指摘として確認済み

## 現状

```objc
} else if (pixelFormat == kCVPixelFormatType_422YpCbCr8_yuvs ||
           pixelFormat == kCVPixelFormatType_422YpCbCr8) {
    // YUY2 形式
```

`convert_pixel_format` も同じ 2 つを `VIDEO_PIXEL_FORMAT_YUY2` に変換している。macOS SDK の `CVPixelBuffer.h` では `kCVPixelFormatType_422YpCbCr8 = '2vuy'`（ordered Cb Y'0 Cr Y'1）と `kCVPixelFormatType_422YpCbCr8_yuvs = 'yuvs'`（ordered Y'0 Cb Y'1 Cr）が別定義である。

## 設計方針

- '2vuy' を YUY2 として扱わない。`PixelFormat` に対応する variant を追加するか、'2vuy' は列挙・キャプチャの対象外として除外する
- 'yuvs' のみを YUY2 として扱う

## 完了条件

- `VideoDevice::formats()` が '2vuy' を YUY2 として報告しない
- '2vuy' のみに対応するカメラで、列挙結果と実際にキャプチャ可能なフォーマットが一致する
- YUY2（'yuvs'）の既存挙動が変わらない
- `cargo clippy --workspace --all-targets -- -D warnings` が通る

## 解決方法

{完了時に記入}
