# 変更履歴

- UPDATE
  - 後方互換がある変更
- ADD
  - 後方互換がある追加
- CHANGE
  - 後方互換のない変更
- FIX
  - バグ修正

## develop


## 2026.1.0

**リリース日**: 2026-07-22

- [ADD] Windows で `CoInitializeEx` / `CoUninitialize` を対で呼び出す RAII ガード `CoInitGuard` を追加し、既存の呼び出しを置き換える
  - @melpon
- [ADD] Linux で `v4l2` と `pipewire` の feature flag を同時に有効化可能にする
  - @melpon
- [ADD] 全プラットフォームでバックエンドを feature flag で制御可能にする
  - macOS AVFoundation に `avf` feature、Windows Media Foundation に `mf` feature を追加
  - `default-*` feature flag で `VideoDeviceList::enumerate()` および `VideoCapture::new()` によるデフォルトバックエンドを選択可能にする
  - @melpon
- [CHANGE] PixelFormat に Mjpeg バリアントを追加し、Linux (V4L2) で mjpeg feature による MJPEG パススルーキャプチャに対応する
  - @voluntas
- [CHANGE] `VideoDevice`, `VideoDeviceList`, `VideoCapture` を構造体から enum に変更する
  - @melpon
- [CHANGE] Windows でキャプチャスレッドが panic したあと再 `start` すると `Error::CaptureFaulted` を返すようにし、stderr にもログを出す
  - @voluntas
- [FIX] `VideoCapture`, `PixelBuffer` はスレッドセーフな構造体ではないので Sync を削除する
  - @melpon
- [FIX] V4L2 バックエンドで I420 フォーマットの場合にコールバックが発生しないのを修正する
  - @melpon
- [FIX] macOS の VideoSession のメンバーがリークしていたのを修正する
  - @melpon
- [FIX] Windows の IMFActivate がリークしていたのを修正する
  - @melpon
- [FIX] Windows で VideoCapture 構築途中失敗時に Media Foundation の参照カウントが残るのを修正する
  - @melpon
- [FIX] PipeWire バックエンドで pw_init に対する pw_deinit が呼ばれずリソースが残るのを修正する
  - @melpon
- [FIX] PipeWire でデバイスのフォーマット列挙が常に空だったのを修正する
  - fps は本修正では仮値 (1.0 / 30.0) で埋まり、choice 形式の正確な fps 抽出は別途対応
  - @melpon
- [FIX] PipeWire でキャプチャ開始時にストリームエラー検出漏れで無限ループするのを修正し、フォーマット指定を choice ベースの範囲指定に変更して交渉成功率を向上させた
  - @melpon

### misc

- [CHANGE] `windows-2025` を `windows-2025-vs2026` にリネームする
  - @voluntas

## 2026.1.0

**リリース日**: 2026-04-03
