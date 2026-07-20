# PipeWire バックエンドで stop→start を繰り返すと core と stream がリークする

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-pipewire-stop-start-leak
- Polished: {YYYY-MM-DD}

## 目的

PipeWire バックエンドの `video_pipewire_session_stop`（`src/video_pipewire.c:945-961`）が `pw_stream_disconnect` のみで stream の destroy も core の disconnect も行わない。`video_pipewire_session_start`（:801-942）は新規に `pw_context_connect` と `pw_stream_new` を呼ぶため、stop→start を繰り返すと旧 core 接続と旧 stream が永久にリークする。このリソースリークを修正する。

## 優先度根拠

- High。stop→start を N 回繰り返すと N 個の core 接続と N 個の stream がリークする
- `VideoCapture::start` / `stop` の公開 API は「stop 後の再 start は許容する」と契約しており、通常の利用パターンで発生する
- `/review-code` の致命的指摘として確認

## 現状

`video_pipewire_session_stop`（`src/video_pipewire.c:945-961`）:

```c
void video_pipewire_session_stop(struct VideoSession* session) {
    if (!session || !atomic_load(&session->running)) {
        return;
    }
    atomic_store(&session->running, 0);
    pw_thread_loop_lock(session->thread_loop);
    if (session->stream) {
        pw_stream_disconnect(session->stream);  // disconnect のみ
    }
    pw_thread_loop_unlock(session->thread_loop);
    pw_thread_loop_stop(session->thread_loop);
}
```

`video_pipewire_session_start`（`src/video_pipewire.c:822-853`）:

```c
session->core = pw_context_connect(session->context, NULL, 0);  // 新規接続
// ...
session->stream = pw_stream_new(session->core, "video-capture", props);  // 新規ストリーム
```

stop 後の再 start で旧 `session->core`（接続済み）と旧 `session->stream`（切断済み・未 destroy）のポインタが上書きされ、永久にリークする。

一方、`video_pipewire_session_destroy`（:770-799）は stream の destroy と core の disconnect を正しく行う。stop と destroy で非対称なクリーンアップになっている。

## 設計方針

`video_pipewire_session_stop` で stream と core を完全に破棄する。再 start 時に新規に生成し直す。

1. `pw_stream_disconnect` の後、`spa_hook_remove(&session->stream_listener)` と `pw_stream_destroy(session->stream)` を呼び、`session->stream = NULL` にする
2. `spa_hook_remove(&session->core_listener)` と `pw_core_disconnect(session->core)` を呼び、`session->core = NULL` にする
3. 再 start 時に `session->core == NULL` / `session->stream == NULL` から新規生成する（既存の start ロジックは NULL チェックなしで新規生成するため、そのまま動作する）
4. `atomic_store(&session->format_ready, 0)` と `atomic_store(&session->stream_error, 0)` を stop 時にリセットする（再 start 時の状態待ちが正しく動作するため）

### 採用しない案

- stop で disconnect のみ行い、start で既存の stream/core を再利用する: PipeWire の stream は disconnect 後の再接続が保証されていない。新規生成が確実

## 完了条件

- `video_pipewire_session_stop` で stream と core を完全に破棄する
- stop→start を繰り返してもリソースがリークしない
- `cargo build --workspace --features pipewire`（Linux）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
