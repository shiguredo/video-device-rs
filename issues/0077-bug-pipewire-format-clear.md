# PipeWire が SPA_PARAM_Format の解除を無視して古い形式を使い続ける

- Priority: Medium
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-format-clear
- Polished: {YYYY-MM-DD}

## 目的

PipeWire バックエンドで、producer が `SPA_PARAM_Format` を解除したときに、
古い `negotiated_width` / `negotiated_height` / `negotiated_format` を使い続ける
状態をなくす。解除後にフレームが届くと、実際の寸法・形式と異なる解釈で
スライスを作るか、サイズ検証で全フレームを破棄する。

## 優先度根拠

- Medium。フォーマットの再ネゴシエーションは通常のストリーミング中に起こりうる。
  解除を無視すると「フレームが来ない」「寸法が違う」という無音故障になり、
  利用者に原因が伝わらない
- PipeWire の公式 example は `param == NULL` を「format のクリア」として扱う

## 現状

`src/video_pipewire.c` の `on_param_changed` は、`param` が `NULL` のとき
先頭で return する。

```c
if (id != SPA_PARAM_Format || !param) {
    return;
}
```

このため `format_ready` は 1 のまま、`negotiated_*` も直前の値のまま残る。
`on_process` は古い寸法と形式で `required_bytes_*` を計算し、バッファを検証する。

## 設計方針

- `id == SPA_PARAM_Format` かつ `param == NULL` のとき、`format_ready` を 0 に戻し、
  `negotiated_width` / `negotiated_height` / `negotiated_format` を初期値へ戻す
- ストリーミング中の解除は、利用者に通知できる経路へ乗せる。`start()` の待機中なら
  失敗として返し、動作中なら `stream_error` を立てて次の `start()` / `stop()` で
  検知できるようにする。既存の通知方針と整合させる
- `on_process` は `format_ready == 0` の間バッファをキューへ戻す既存の動作を維持する

## 完了条件

- `SPA_PARAM_Format` の解除後に、古い `negotiated_*` を使ったフレームが
  コールバックへ渡らない
- 解除後に新しい形式が通知された場合、その形式でフレームが再開する
- ストリーミング中の解除が、エラーまたは状態として利用者に検知できる
- Linux 実機で、再ネゴシエーションを発生させて確認する

## 解決方法

{完了時に記入}
