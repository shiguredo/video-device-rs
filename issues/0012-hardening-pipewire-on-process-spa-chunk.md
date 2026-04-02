# PipeWire `on_process` で `spa_buffer` の `chunk` を参照する前に NULL チェックする

Created: 2026-04-02  
Model: Composer 1

## なぜこの対応が必要か

`spa_buf->datas[0].data` は NULL チェック済みだが、**`spa_buf->datas[0].chunk->stride`** は **`chunk` が NULL** のとき未定義動作になり得る。

## 現状コード（調査結果）

### 参照ファイル

- `src/video_pipewire.c`

### `on_process`（約 305〜377 行）

- 322〜326 行: `spa_buf->datas[0].data` が NULL なら `pw_stream_queue_buffer` して return。
- **328〜329 行**:

```c
    const uint8_t* data = spa_buf->datas[0].data;
    int stride = spa_buf->datas[0].chunk->stride;
```

- **`chunk` の NULL チェックがない**。
- `session` 構造体に **`negotiated_stride`** フィールド（`video_pipewire.c` 約 42 行）があるが **`on_param_changed` で未設定**（現状未使用）。**フォールバックの stride 源**として使えるかは **PipeWire の仕様調査**が必要。

## 提案する実装（この issue だけで完結）

### ステップ 1: NULL 防御（必須）

`329` 行より前に追加:

```c
    if (!spa_buf->datas[0].chunk) {
        pw_stream_queue_buffer(session->stream, buf);
        return;
    }
    int stride = spa_buf->datas[0].chunk->stride;
```

- **`stride` が負または 0** の場合も、以降の `y_size = stride * height` が危険なので、**`stride <= 0` なら同様に queue して return** するか、**`on_param_changed` で取った幅から推定**するかを **コメントで方針決め**。

### ステップ 2（任意）: `negotiated_stride` の活用

- `on_param_changed`（約 276〜303 行）で **`spa_format_video_raw_parse` 後**に **`info.info.raw.stride`** 等が取れるか **SPA ヘッダを確認**。
- 取れるなら **`session->negotiated_stride` に格納**し、`chunk` が NULL のとき **フォールバック**として使う。

### ステップ 3: コメント

- **`chunk` が常に非 NULL である**ことが **PipeWire バージョン X で保証される**なら、その**根拠（URL またはヘッダ名）**を **日本語コメント**で `video_pipewire.c` に残し、**NULL チェックを省略する**判断も可能（**調査結果を issue 本文に追記**すること）。

## テスト・検証

- **PipeWire 環境**でカメラキャプチャを実行し、**クラッシュなし**。
- **NULL 分岐**を入れた場合、**通常フレームでパスが通る**ことをログまたは一時的なカウンタで確認（本番マージ前に削除）。

## 完了条件（チェックリスト）

- [ ] `chunk` NULL または **不正 stride** 時に **セグフォしない**（防御コードまたは根拠コメント）。
- [ ] 変更意図が **日本語コメント**で分かる。

## 依存関係

- **なし**。

## 関連ファイル一覧

| ファイル | 変更想定 |
|----------|----------|
| `src/video_pipewire.c` | `on_process`、任意で `on_param_changed` |
