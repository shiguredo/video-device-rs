# V4L2 `init_mmap` が途中失敗したときの mmap / buffers のリーク

Created: 2026-04-02  
Model: Composer 1  
Completed: 2026-04-02

## なぜこの対応が必要か

`video_session_create` が `NULL` を返す経路でも、**既に `mmap` した領域や `calloc` した `session->buffers` が解放されない**可能性がある。**fd を閉じた後もカーネルに mmap 領域が残る**（プロセス終了まで）**ユーザ空間の `Buffer` 配列はリーク**する。

## 現状コード（調査結果）

### 参照ファイル

- `src/video_v4l2.c`

### `init_mmap`（約 345〜388 行）

1. `VIDIOC_REQBUFS` 失敗（352〜354 行）→ **`session->buffers` は未割当** → リークなし。
2. `req.count < 2`（356〜358 行）→ 同上。
3. `calloc` 失敗（360〜363 行）→ 同上。
4. ループ内で **`VIDIOC_QUERYBUF` 失敗**（374〜376 行）→ **`session->buffers` は確保済み**、**`i` 未満のインデックスは `mmap` 成功済みの可能性** → **以降の mmap を `munmap` せず `return -1`**。
5. **`mmap` が `MAP_FAILED`**（382〜384 行）→ 同上（**同じインデックス**は `length` 未設定のままの可能性あり。`cleanup_mmap` は `start && != MAP_FAILED` で `munmap` するので、**部分初期化の `buffers[i]` をゼロ初期化**していれば `calloc` 済みなので `start` は NULL か MAP_FAILED）。

### `video_session_create` の失敗パス（約 483〜486 行）

```c
    if (init_mmap(session) < 0) {
        close(fd);
        free(session);
        return NULL;
    }
```

- **`cleanup_mmap(session)` を呼ばない**ため、`session->buffers` が非 NULL かつ **一部 `mmap` 済み**のとき **`munmap` / `free(session->buffers)` が行われない**。

### 既存の `cleanup_mmap`（約 390〜400 行）

- `session->buffers` の各要素で `start` が有効なら `munmap` し、`free(session->buffers)` する。
- **`video_session_destroy`（492 行以降）** は `stop` のあと `cleanup_mmap` を呼ぶため、**正常系の解放は問題ない**。

## 提案する実装（この issue だけで完結できる手順）

### 方針 A（推奨）: `init_mmap` 失敗時に `cleanup_mmap` を呼ぶ

1. `init_mmap` の先頭付近で **`session->buffers` / `buffer_count` を触る前**に `session->buffers == NULL` を想定していることを確認。
2. **`return -1` の直前**（374〜376, 382〜384 行の各箇所）で、**`session` が部分初期化済みなら** `cleanup_mmap(session)` を呼ぶ。  
   - 注意: `cleanup_mmap` は `session->buffers` を NULL にする。呼び出し後は **`buffer_count` も 0 に戻す**か、`cleanup_mmap` 内で `buffer_count = 0` をセットする（現状は `buffers = NULL` のみ）。
3. **ループの `return -1` が 2 箇所**あるため、**重複を避けるなら** `goto fail` で `fail:` ラベルに **`cleanup_mmap(session); return -1;`** を1か所にまとめる。

### 方針 B: `video_session_create` の失敗パスで `cleanup_mmap` を呼ぶ

- `init_mmap(session) < 0` のブロック内で **`cleanup_mmap(session)` を `free(session)` の前**に呼ぶ。  
- ただし **`init_mmap` 内で `buffer_count` が未設定のまま失敗するケース**（例: `calloc` 直後の `QUERYBUF` 失敗）では、`cleanup_mmap` は `session->buffers` があれば解放するので、**`buffer_count` が未設定だとループが回らない**。`init_mmap` では **`session->buffer_count = req.count` を `calloc` 成功直後**（365 行付近）に設定済みなので、`cleanup_mmap` の `for` は `buffer_count` 回回る。**未 mmap の `start` は 0**（`calloc`）なので、`cleanup_mmap` の `if (session->buffers[i].start && ...)` でスキップされる。  
- **方針 A の方が責務が `init_mmap` に閉じて分かりやすい**。

### `video_session_create` 側の修正

- 方針 A を採用した場合、`init_mmap` が **内部で `cleanup_mmap` を呼んだあと `return -1`** なら、`session` には **`buffers == NULL`** が入る想定にする。  
- `video_session_create` の `init_mmap` 失敗時は **従来どおり `close(fd); free(session);`** でよい（**`buffers` は NULL**）。

## テスト・検証

- **ユニットテスト**は C を直接叩きにくい場合、`init_mmap` を **静的関数のまま**テスト用に切り出すか、**失敗時に `session->buffers` が NULL になる**ことを `video_session_destroy` 不要のパスで確認する**統合テスト**はコストが高い。  
- **最低限**: コードレビューで **すべての `return -1` が `cleanup` または `goto fail` 経由**であること。
- **可能なら**: `valgrind` または `asan` で **短いキャプチャセッション**を実行し、**リークレポートが増えない**ことを確認。

## 完了条件（チェックリスト）

- [ ] `init_mmap` が途中で失敗しても、**確保済みの `mmap` がすべて `munmap`** され、**`session->buffers` が `free` される**（または失敗後に `session->buffers == NULL` で `video_session_create` が `free(session)` だけでよい状態）。
- [ ] `video_session_destroy` の既存パスを壊していない（**回帰手動確認**: キャプチャ開始→停止→破棄）。
- [ ] 変更箇所に**日本語コメント**で「部分失敗時の解放」を1行以上書く（`AGENTS.md` に従う）。

## 依存関係

- **他 issue なし**。`closed/0004` 等の `from_raw_parts` 議論とは独立。

## 関連ファイル一覧

| ファイル | 変更想定 |
|----------|----------|
| `src/video_v4l2.c` | `init_mmap`、必要なら `cleanup_mmap` のコメント |

## 解決方法

### 問題の整理

`init_mmap` がループ途中で失敗した場合、既に `mmap` したバッファと `calloc` した `session->buffers` が残ったまま `return -1` していた。`video_session_create` の失敗パスは `close(fd); free(session);` のみで **`cleanup_mmap` を呼ばない**ため、`munmap` と `buffers` の `free` が行われずリーク・リソース残存になっていた。

### 実装内容

1. **`init_mmap` 内の失敗経路を `fail:` に集約**（`src/video_v4l2.c`）。`VIDIOC_QUERYBUF` が失敗したとき（374〜376 行付近）と、`mmap` が `MAP_FAILED` のとき（382〜384 行付近）は `goto fail` とした。
2. **`fail:` で `cleanup_mmap(session)` を呼び、その後 `return -1`**（389〜392 行）。既存の `cleanup_mmap` が各 `mmap` を `munmap` し、`free(session->buffers)` して `session->buffers = NULL` にする。
3. **`cleanup_mmap` で `session->buffer_count = 0` をセット**（403〜404 行）。部分初期化後も `buffer_count` とループ回数が一致するようにした。
4. **日本語コメント**で「部分失敗時: 既に mmap した領域と buffers 配列を解放する」と記載した（390 行付近）。
5. **`video_session_create` 側**は、`init_mmap` 失敗後に `session->buffers` が NULL になるため、従来どおり `close(fd); free(session);` でよい（変更なし）。

### 検証の目安

`REQBUFS` 失敗・`req.count < 2`・`calloc` 失敗の経路では従来どおり `buffers` 未割当のため `cleanup_mmap` は実質 no-op。問題だったのは **`calloc` 成功後のループ内失敗**のみ。
