# PipeWire で明示したピクセルフォーマットが choice の代替で無音に置換される

- Priority: Medium
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-requested-format-choice
- Polished: {YYYY-MM-DD}

## 目的

PipeWire バックエンドで `pixel_format: Some(Nv12)` のように形式を明示しても、
ネゴシエーションで YUY2 や I420 が選ばれ得る状態を直す。要求と違う形式が
エラーなしに返ると、NV12 前提で stride やプレーンを解釈する利用者が誤動作する。

## 優先度根拠

- Medium。公開 API は「取得するピクセルフォーマット」と読める指定を提供しており、
  明示指定が best-effort であることは doc にも書かれていない
- `VideoFrame.pixel_format` を見れば実際の形式は分かるが、要求と違うことに
  気づかない利用者がいる

## 現状

`src/video_pipewire.c` の `video_pipewire_session_start` は、
`SPA_FORMAT_VIDEO_format` を次の choice で組み立てる。

```c
SPA_POD_CHOICE_ENUM_Id(4,
    session->requested_format,
    session->requested_format,
    SPA_VIDEO_FORMAT_YUY2,
    SPA_VIDEO_FORMAT_I420)
```

`requested_format` は `pixel_format` を指定した場合も、`None` の場合の既定値も
同じ経路で入る。choice の 3 番目以降が代替になるため、明示指定しても
YUY2 / I420 が選ばれ得る。

## 設計方針

- `VideoCaptureConfig.pixel_format` が `Some` のときは、代替を含まない単一値
  （`SPA_POD_Id`）で要求する。サーバがその形式を扱えなければ `start()` を
  エラーにする
- `pixel_format: None` のときは、従来どおり choice で複数候補を提示する
- フレームレートとサイズの choice は変更しない
- 要求が best-effort であることを許容する設計にする場合は、その契約を
  公開 doc と `VideoFrame.pixel_format` の説明に明記する。単一値方式を優先する

## 完了条件

- `pixel_format: Some(Nv12)` で開始した場合、NV12 のフレームが届くか、
  `start()` が明示的なエラーを返す
- `pixel_format: None` の既定設定では、従来どおり複数候補からネゴシエーションできる
- 要求と違う形式が無音で返らないことを Linux 実機で確認する
- 既存の NV12 / YUY2 / I420 の経路が変わらない

## 解決方法

{完了時に記入}
