# V4L2 の既定フォーマット選択が I420 / MJPEG にフォールバックしない

- Priority: Medium
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-v4l2-default-format-fallback
- Polished: {YYYY-MM-DD}

## 目的

V4L2 バックエンドで `pixel_format: None` を指定したときに、I420 または MJPEG
しか提供しないカメラでもキャプチャを開始できるようにする。現在の既定選択は
NV12 と YUY2 しか試さず、両方失敗するとセッション作成を諦める。

## 優先度根拠

- Medium。I420 のみを出すカメラは限られるが、対応フォーマットとして変換表に
  存在し、明示指定すれば動く経路があるのに既定設定では開けない。バックエンド間の
  挙動の非対称でもある
- `src/video_v4l2.c` の `convert_video_pixel_format_to_v4l2` が I420 と MJPEG を
  変換できることを確認済み

## 現状

`src/video_v4l2.c` の `video_v4l2_session_create` は、`requested_pixel_format` が
`0` のとき次の順で `VIDIOC_S_FMT` を試す。

1. `V4L2_PIX_FMT_NV12`
2. `V4L2_PIX_FMT_YUYV`

どちらも設定できなければ `close(fd)` して `NULL` を返す。`V4L2_PIX_FMT_YUV420`
（I420）と、`SHIGUREDO_VIDEO_DEVICE_MJPEG` 定義時の `V4L2_PIX_FMT_MJPEG` は
試さない。

## 設計方針

- 既定選択の候補に I420 を加える。MJPEG は `mjpeg` feature 有効時のみ候補に加える
- 優先順は NV12、YUY2、I420、MJPEG とする。カメラが複数対応する場合の従来の
  選択結果を変えない
- 明示指定 (`requested_pixel_format != 0`) の厳格さは変えない。指定形式が
  設定できなければ従来どおり失敗する
- 候補ごとに `VIDIOC_S_FMT` を試す現在の構造を保ち、成功した `fmt` から
  `width` / `height` / `pixel_format` を保存する

## 完了条件

- I420 のみを提供する V4L2 デバイスを `pixel_format: None` で開ける
- `mjpeg` feature 有効時、MJPEG のみを提供するデバイスを `pixel_format: None` で
  開ける
- NV12 / YUY2 対応デバイスで、従来と同じ形式が選ばれる
- 明示指定した形式が使えない場合のエラー挙動が変わらない
- Linux 実機の device test で、既定設定のキャプチャが通る

## 解決方法

{完了時に記入}
