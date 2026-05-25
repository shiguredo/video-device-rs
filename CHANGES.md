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

- [ADD] Windows で `CoInitializeEx` / `CoUninitialize` を対で呼び出す RAII ガード `CoInitGuard` を追加し、既存の呼び出しを置き換える
  - @melpon
- [FIX] `VideoCapture`, `PixelBuffer` はスレッドセーフな構造体ではないので Sync を削除する
  - @melpon
- [FIX] V4L2 バックエンドで I420 フォーマットの場合にコールバックが発生しないのを修正する
  - @melpon
- [FIX] macOS の VideoSession のメンバーがリークしていたのを修正する
  - @melpon

### misc


## 2026.1.0

**リリース日**: 2026-04-03
