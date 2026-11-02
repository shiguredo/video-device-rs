# PipeWire の VideoFormat が常に min_fps 1.0 / max_fps 30.0 の固定値を返す

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-fps-fabricated
- Polished: {YYYY-MM-DD}

## 目的

`src/video_pipewire.c` の `on_node_param` は、デバイスの対応フォーマットを列挙する際に fps を取得せず、`min_fps = 1.0f` / `max_fps = 30.0f` を固定値で埋めている。`VideoFormat` は公開 API であり、利用者は「このフォーマットは 1〜30fps 対応」と誤認する。実際には 60fps で交渉しうる。

コード内のコメントは「0.0 は CI の Device Test ジョブが `all(.max_fps > 0)` を要求しているため使えない」としており、テスト側の条件が実装の不正確さを固定化している。

## 優先度根拠

- Medium。公開 API が実測値ではない値を返す。誤った fps で機器選択・設定を行う利用者に実害がある
- CHANGES.md には既知の制限として書かれているが、公開 API の doc には無く、追跡 issue も無い
- `/review-code` の重要指摘として確認済み

## 現状

```c
// fps は未取得のため仮値を埋める
// - 0.0 は CI の Device Test ジョブが all(.max_fps > 0) を要求しているため使えない
// - 正確な fps 抽出 (choice 形式の展開) は別途対応
p->formats[p->format_count].min_fps = 1.0f;
p->formats[p->format_count].max_fps = 30.0f;
```

`SPA_FORMAT_VIDEO_framerate` の choice pod（range / enum）を展開していない。

## 設計方針

- `spa_pod` の choice を展開して実際の min / max を求める
- 取得できない場合の表現を決める（0.0 を許容する仕様にしてテストと CI の条件を修正する、または「不明」を表す値を doc で定義する）
- CI の jq 条件（`max_fps > 0`）を実装ではなく API 契約に合わせて見直す

## 完了条件

- PipeWire で列挙した `VideoFormat` の `min_fps` / `max_fps` が、デバイスが実際に提示する範囲を反映する
- 実測できない場合の挙動が公開 doc に明記されている
- `tests/test_pipewire.rs` と `.github/workflows/ci.yml` の fps 条件が実装と矛盾しない
- Linux 実機の device test が通る

## 解決方法

{完了時に記入}
