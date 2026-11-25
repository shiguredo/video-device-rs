# PipeWire が無効な PTS を有効値として扱い、タイムスタンプが 0 になる

- Priority: Medium
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-invalid-pts
- Polished: {YYYY-MM-DD}

## 目的

PipeWire バックエンドで、producer が `spa_meta_header.pts` に無効値を入れた場合に、
`VideoFrame.timestamp_us` が 0 になる問題を直す。0 が全フレームに並ぶと、
利用側の同期・遅延計測・フレーム間引きが破綻する。

## 優先度根拠

- Medium。PipeWire の公式 example は無効値として `-1` を代入しており、
  実在する producer で再現しうる
- タイムスタンプは公開 API の一部で、0 は「有効な時刻」と区別できない

## 現状

`src/video_pipewire.c` の `on_process` は、`SPA_META_Header` の `pts` を次の条件で
有効とみなす。

```c
if (header && header->pts != (int64_t)INT64_MIN) {
    timestamp_us = header->pts / 1000;
} else {
    /* CLOCK_MONOTONIC にフォールバック */
}
```

`spa_meta_header.pts` には無効値を表す定義済み sentinel が無く、公式 example は
`-1` を使う。`-1 / 1000` は 0 になるため、無効値が「時刻 0」として利用者へ渡る。
`INT64_MIN` だけを無効とする現在の判定では `-1` を弾けない。

## 設計方針

- `pts <= 0` を無効として扱い、`clock_gettime(CLOCK_MONOTONIC)` の値へ
  フォールバックする。SPA が sentinel を定義していないことをコメントに残す
- フォールバック側のマイクロ秒変換は既存のまま使う
- タイムスタンプの起点がバックエンド依存であることは公開 doc の範囲とし、
  この issue では値の妥当性だけを扱う

## 完了条件

- `pts` が `-1` または 0 のフレームで、`timestamp_us` が単調時計由来の正の値になる
- `pts` が正のフレームでは、従来どおりナノ秒からマイクロ秒へ変換した値になる
- Linux 実機で、`pts` に負値を入れる producer を使って 0 が並ばないことを確認する

## 解決方法

{完了時に記入}
