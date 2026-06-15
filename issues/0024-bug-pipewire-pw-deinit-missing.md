# PipeWire 経路で pw_init に対する pw_deinit が呼ばれずリソースが残る

- Priority: Medium
- Created: 2026-06-15
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/fix-pipewire-pw-deinit-missing
- Polished: 2026-06-15

## 目的

`src/video_pipewire.c` の 2 箇所で呼ばれている `pw_init(NULL, NULL)` に対する `pw_deinit()` がリポジトリ全体で一度も呼ばれておらず、PipeWire の内部リソースがプロセス終了まで残る。すべての成功・失敗経路で `pw_init` と `pw_deinit` を 1:1 で対にする。

## 優先度根拠

- Medium。PipeWire の `pw_init` は内部で参照カウントを取り、最初の呼び出しでスレッドやプロトコル設定を確保する。`pw_deinit` を呼ばないと:
  - 列挙・キャプチャを繰り返すたびに参照カウントが増え続け、`pw_deinit` のチャンスが無いままプロセスが終わる
  - プロセス長期実行時の地味なリーク (メモリ・ファイルディスクリプタ)
- 即クラッシュではないが、`pw_init` / `pw_deinit` の対称性は PipeWire 公式 API の契約
- `/review-code` の致命的指摘として報告されたが、影響度としては Medium 寄り (露見しにくいが、契約違反として確実に修正すべき)
- PipeWire の `pw_init` / `pw_deinit` は内部で原子的に参照カウントを増減するスレッドセーフな API。enumerate と session が並列に動いた場合でも、本 issue で「`pw_init` を呼んだら必ず対応する `pw_deinit` を呼ぶ」契約を守れば内部状態の一貫性は保たれる

## 現状

`src/video_pipewire.c:148` (`video_pipewire_enumerate_devices`):

```c
pw_init(NULL, NULL);
struct EnumerateContext ctx = {0};
// ...
```

`src/video_pipewire.c:575` (`video_pipewire_session_create`):

```c
pw_init(NULL, NULL);
// デフォルト値の設定 ...
```

リポジトリ全体を `grep "pw_deinit"` してもヒットゼロ。`video_pipewire_free_devices` / `video_pipewire_session_destroy` のいずれにも対応する `pw_deinit` 呼び出しがない。

## 設計方針

すべての失敗・成功経路に `pw_deinit()` を直接追加する最小変更案を採用する。`goto cleanup;` パターンや static helper への切り出しは本 issue では行わない (refactor は別 issue で扱う方針)。

### 1. `video_pipewire_enumerate_devices` (`src/video_pipewire.c:140-204`) の修正

`pw_init` の位置 (L148、`calloc` 不要の関数なので `pw_init` 後すぐ `EnumerateContext` を初期化) は維持する。

修正対象の経路は以下 5 つ。

| 経路 | 場所 | 現状 |
| --- | --- | --- |
| 正常 return | L203 (`return 0;` の直前、cleanup ブロック L193-199 の後) | `pw_deinit` なし |
| `pw_main_loop_new` 失敗 | L153-155 | `pw_deinit` なし |
| `pw_context_new` 失敗 | L159-162 | `pw_deinit` なし |
| `pw_context_connect` 失敗 | L165-169 | `pw_deinit` なし |
| `pw_core_get_registry` 失敗 | L177-182 | `pw_deinit` なし |

それぞれ `return` の直前に `pw_deinit();` を追加する。例 (`pw_core_get_registry` 失敗):

```c
ctx.registry = pw_core_get_registry(ctx.core, PW_VERSION_REGISTRY, 0);
if (!ctx.registry) {
    pw_core_disconnect(ctx.core);
    pw_context_destroy(ctx.context);
    pw_main_loop_destroy(ctx.loop);
    pw_deinit();
    return -4;
}
```

正常 return 部分:

```c
// クリーンアップ
spa_hook_remove(&ctx.registry_listener);
pw_proxy_destroy((struct pw_proxy*)ctx.registry);
spa_hook_remove(&ctx.core_listener);
pw_core_disconnect(ctx.core);
pw_context_destroy(ctx.context);
pw_main_loop_destroy(ctx.loop);
pw_deinit();

*devices = ctx.devices;
*count = ctx.count;
return 0;
```

### 2. `video_pipewire_session_create` (`src/video_pipewire.c:567-622`) の修正

`pw_init` の呼び出し位置 (L575、`calloc` 成功後) は維持する。`calloc` 失敗 (L571-573) は `pw_init` 前なので `pw_deinit` は不要。

修正対象の経路は `pw_init` 後の失敗 return 3 つ。

| 経路 | 場所 | 現状 |
| --- | --- | --- |
| `requested_format == SPA_VIDEO_FORMAT_UNKNOWN` | L592-595 | `pw_deinit` なし |
| `pw_thread_loop_new` 失敗 | L606-610 | `pw_deinit` なし |
| `pw_context_new` 失敗 | L614-619 | `pw_deinit` なし |

それぞれ `return NULL;` の直前に `pw_deinit();` を追加する。例 (`pw_context_new` 失敗):

```c
session->context = pw_context_new(
    pw_thread_loop_get_loop(session->thread_loop), NULL, 0);
if (!session->context) {
    pw_thread_loop_destroy(session->thread_loop);
    free(session->device_id);
    free(session);
    pw_deinit();
    return NULL;
}
```

正常 return (`return session;`、L621) では `pw_deinit` を呼ばない。`pw_init` の参照カウントは `video_pipewire_session_destroy` でデクリメントする。

### 3. `video_pipewire_session_destroy` (`src/video_pipewire.c:624-652`) の修正

冒頭の null check (`if (!session) return;`) は維持する。`session == NULL` のときは対応する `pw_init` 呼び出しが行われていないため `pw_deinit` を呼ばない。

末尾の `free(session);` の直前に `pw_deinit();` を追加する。

```c
void video_pipewire_session_destroy(struct VideoSession* session) {
    if (!session) {
        return;
    }
    // 既存の停止・解放処理 ...
    free(session->device_id);
    pw_deinit();
    free(session);
}
```

`session_create` が成功して返した `session` は必ず本関数で破棄される契約のため、`pw_init` ↔ `pw_deinit` が常に 1:1 になる。

## 影響範囲

- `src/video_pipewire.c`: `enumerate_devices` の 5 経路、`session_create` の 3 経路、`session_destroy` の 1 経路に `pw_deinit();` を追加 (計 9 箇所)
- `src/video_pipewire.h`, `src/capture_ffi.rs`, `src/device_ffi.rs` 等の Rust 側: 変更なし (公開 ABI は不変)
- 公開 API: 変更なし
- 0025 (`enumerate_devices` のフォーマット情報取得) と同じ `src/video_pipewire.c` を触るが、本 issue は `pw_init` / `pw_deinit` 行の追加のみで `EnumerateContext` 構造体・`enum_registry_global` には触らない。本 issue を先にマージし、0025 はその上にリベースする方針を採る。コンフリクトが起きた場合は手動でマージする
- スコープ外: `session_create` 内の `session->device_id = device_id ? strdup(device_id) : NULL;` (L600) は `strdup` の戻り値を NULL チェックしていないが、後段 (`session->device_id` を使う箇所) で NULL を許容する作りになっているためクラッシュには至らない。`strdup` 失敗の扱いは本 issue のスコープ外で、必要なら別 issue として扱う

## 完了条件

- `video_pipewire_enumerate_devices` のすべての return パス (正常 1 経路、失敗 4 経路) で `pw_init` と `pw_deinit` の呼び出し回数が 1:1 になっていること
- `video_pipewire_session_create` の `pw_init` 後のすべての失敗 return パス (3 経路) で `pw_init` と `pw_deinit` の呼び出し回数が 1:1 になっていること
- `video_pipewire_session_destroy` で `session != NULL` の場合は必ず `pw_deinit()` が呼ばれること。`session == NULL` 早期 return の場合は `pw_deinit` を呼ばないこと
- 検証方法: コードレビュー時にすべての `pw_init` 呼び出しと `pw_deinit` 呼び出しを目視で対照確認する。補助的に `grep -c "pw_init\b" src/video_pipewire.c` と `grep -c "pw_deinit\b" src/video_pipewire.c` で件数を比較する (`pw_init` 2 件、`pw_deinit` 9 件 = `enumerate` 5 + `session_create` 3 + `session_destroy` 1 が想定値)
- 既存の正常系 (列挙成功・セッション正常終了) が引き続き動作する
- Linux PipeWire 環境 (`.github/workflows/ci.yml` の `Ubuntu (pipewire)` および `Ubuntu (v4l2,pipewire)` ジョブ) で `cargo test --workspace` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] PipeWire バックエンドで pw_init に対する pw_deinit が呼ばれずリソースが残るのを修正する`)。担当者行 (`- @<github-id>`) を含める

## 解決方法

{完了時に記入}
