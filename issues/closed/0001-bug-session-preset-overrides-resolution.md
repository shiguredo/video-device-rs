# macOS で sessionPreset が出力解像度を上書きする

## カテゴリ

バグ

## 概要

macOS の AVFoundation では AVCaptureSession のデフォルト sessionPreset が AVCaptureSessionPresetHigh（= 1080p）であり、device.activeFormat で指定した解像度を上書きしてしまう。

## 再現手順

1. 1280x720 など 1080p 以外の解像度でキャプチャセッションを作成する
2. device.activeFormat で正しいフォーマットが選択されていることを確認する
3. 実際に受信されるフレームの解像度を確認する

## 期待される動作

指定した解像度（例: 1280x720）のフレームが返される。

## 実際の動作

sessionPreset のデフォルト値（AVCaptureSessionPresetHigh = 1920x1080）により、常に 1920x1080 のフレームが返される。

## 備考

- iOS では AVCaptureSessionPresetInputPriority を使って activeFormat を優先できるが、macOS ではこの API が利用不可
- AVCaptureVideoDataOutput.videoSettings に kCVPixelBufferWidthKey / kCVPixelBufferHeightKey を指定することで解決可能

## 解決方法

`src/video_c.m` の `video_session_create` 関数で、`output.videoSettings` に `kCVPixelBufferWidthKey` と `kCVPixelBufferHeightKey` を追加し、引数の `width`/`height` を出力解像度として明示指定するようにした。
