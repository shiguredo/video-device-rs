# V4L2 のエラーパスで formats がリークし、エラーフラグ付きフレームを利用者へ渡している

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-v4l2-error-path-resource-and-flags
- Polished: 2026-11-05

## 目的

`src/video_v4l2.c` の 2 つの不備を修正する。

1. `video_v4l2_enumerate_devices` のメモリ不足時後始末で、`device_array[j]->name` と `unique_id` は解放するが `formats` を解放していない。`enumerate_device_formats` が `realloc` で確保した配列がリークする。この後始末は `device_array` の `realloc` 失敗時と `device` の `malloc` 失敗時の 2 箇所にある
2. `capture_thread` は `struct v4l2_buffer` の `flags` を確認せずにフレームをコールバックへ渡す。`V4L2_BUF_FLAG_ERROR` が立った壊れたフレームも利用者へ届く

## 優先度根拠

- Medium。(1) は OOM 時のみ発生するリーク、(2) はドライバがエラーを報告したフレームを利用者が正常フレームとして扱う
- いずれも通常パスでは発生しないが、既存の 0034（V4L2 エラーパスのハードニング）とは対象箇所が異なる追加分
- `/review-code` の指摘として確認済み

## 現状

エラー時の解放ループは次の 3 つの free のみで、`formats` を解放していない。

```c
free(device_array[j]->name);
free(device_array[j]->unique_id);
free(device_array[j]);
```

この後始末は `video_v4l2_enumerate_devices` の `device_array` の `realloc` 失敗時と `device` の `malloc` 失敗時の 2 箇所に同一コードとして存在する。

`capture_thread` は `buf.bytesused` と `buf.index` は確認するが、`buf.flags` は参照していない。MJPEG 用のコメントでは `V4L2_BUF_FLAG_ERROR` を意識した記述があるが、実装には反映されていない。

## 設計方針

- 列挙のエラー後始末の 2 箇所で `formats` も解放する（`video_avf_enumerate_devices` の同等処理と揃える）
- `buf.flags & V4L2_BUF_FLAG_ERROR` の場合はフレームをコールバックせず `VIDIOC_QBUF` でキューへ戻す。`V4L2_BUF_FLAG_ERROR` はデータ破損を示すフラグ（`linux/videodev2.h`）
- 正常フレームの挙動は変えない範囲で追加する

closed/0020 では `V4L2_BUF_FLAG_ERROR` 付きフレームもそのまま渡す方針だったが、本 issue では V4L2 のエラーフレームを破棄する方針に変更する。他バックエンドにはこのフラグがないため影響しない。

## 完了条件

- 列挙のメモリ不足経路の 2 箇所でリークが無い（コードレビューと ASAN で確認）
- `V4L2_BUF_FLAG_ERROR` 付きフレームがコールバックへ渡らない
- 正常フレームの挙動が変わらない
- `cargo test --workspace` が通る

## 解決方法

{完了時に記入}
