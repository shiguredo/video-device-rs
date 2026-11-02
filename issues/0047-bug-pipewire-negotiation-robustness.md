# PipeWire のストリームネゴシエーションで未知フォーマットと範囲外要求が無音で失敗する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-negotiation-robustness
- Polished: {YYYY-MM-DD}

## 目的

`src/video_pipewire.c` の `video_pipewire_session_start` と `on_param_changed` / `on_process` に、ネゴシエーション失敗が利用者に伝わらない経路が 2 つある。

1. 要求した幅・高さ・フレームレートが choice の固定上限（幅 3840、高さ 2160、fps 60）を超えると、`def_size` / `def_fps` が choice の範囲外になる。サーバは範囲内へクランプするか既定値を無視するため、要求と違う解像度・フレームレートがエラーなしで返る。この上限は公開 doc にも書かれていない
2. ネゴシエーション結果が NV12 / YUY2 / I420 以外の場合、`on_param_changed` は `format_ready` を 1 にし、`on_process` は毎回バッファをキューへ戻すだけになる。`start()` は成功を返し、フレームは 1 枚も届かず、ログも状態も残らない

## 優先度根拠

- Medium。いずれも「成功したように見えて要求どおりに動かない」無音故障
- 4K を超える解像度や 60fps 超を要求するカメラ・利用者で再現する
- `/review-code` の指摘として確認済み

## 現状

```c
struct spa_fraction def_fps = SPA_FRACTION(session->requested_fps, 1);
struct spa_fraction max_fps = SPA_FRACTION(60, 1);
struct spa_rectangle def_size = SPA_RECTANGLE(session->requested_width, session->requested_height);
struct spa_rectangle max_size = SPA_RECTANGLE(3840, 2160);
```

`convert_spa_video_format` は NV12 / YUY2 / I420 以外で 0 を返し、`on_process` の `else` 分岐でバッファを戻すだけになる。

## 設計方針

- choice の上限を要求値以上に動的に広げる、または上限超過を `Error::InvalidCaptureConfig` として事前に拒否する
- ネゴシエーション結果が未知フォーマットなら `stream_error` を立てて `start()` を失敗させる、または利用者へ通知する経路を設ける
- 上限値を公開 doc に明記する

## 完了条件

- 要求値が現在の固定上限を超える場合、要求どおりの値で交渉されるか明示的なエラーになる（無音のクランプが起きない）
- 未知フォーマットが選ばれた場合に `start()` が失敗するか、ログと状態で検知できる
- 既存の NV12 / YUY2 / I420 の経路が変わらない
- Linux 実機の device test が通る

## 解決方法

{完了時に記入}
