# PipeWire バックエンドで stop→start を繰り返すと core と stream がリークする

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-pipewire-stop-start-leak
- Polished: 2026-07-21

## 目的

PipeWire バックエンドの `video_pipewire_session_stop`（`src/video_pipewire.c:945-961`）が `pw_stream_disconnect` のみで stream の destroy も core の disconnect も行わない。`video_pipewire_session_start`（:801-943）は新規に `pw_context_connect` と `pw_stream_new` を呼ぶため、stop→start を繰り返すと旧 core 接続と旧 stream が永久にリークする。このリソースリークを修正する。

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

`video_pipewire_session_start`（`src/video_pipewire.c:822-854`）:

```c
session->core = pw_context_connect(session->context, NULL, 0);  // 新規接続
// ...
session->stream = pw_stream_new(session->core, "video-capture", props);  // 新規ストリーム
```

stop 後の再 start で旧 `session->core`（接続済み）と旧 `session->stream`（切断済み・未 destroy）のポインタが上書きされ、永久にリークする。

## 設計方針

`video_pipewire_session_stop` で stream と core を完全に破棄する。再 start 時に新規に生成し直す。stream/core の破棄操作は `pw_thread_loop_lock` / `unlock` の内側で実行し、`pw_thread_loop_stop` はロック解除後に呼ぶ（既存コード :958-960 と同じ構造）。

PipeWire の thread loop は「外部ロック保持中、ループはイテレートしない」セマンティクスを持つ。`pw_thread_loop_lock` は実行中のコールバック（`on_process` 等）の完了を待ってからロックを取得するため、ロック内で `pw_stream_destroy` を呼んでも `on_process` が `session->stream` を参照する UAF は起きない。

1. `pw_stream_disconnect` の後、`spa_hook_remove(&session->stream_listener)` と `pw_stream_destroy(session->stream)` を呼び、`session->stream = NULL` にする
2. `spa_hook_remove(&session->core_listener)` と `pw_core_disconnect(session->core)` を呼び、`session->core = NULL` にする
3. 再 start 時に `session->core == NULL` / `session->stream == NULL` から新規生成する（既存の start ロジックは NULL チェックなしで新規生成するため、そのまま動作する）
4. `atomic_store(&session->format_ready, 0)` と `atomic_store(&session->stream_error, 0)` を stop 時にリセットする（再 start 時の状態待ちが正しく動作するため）。0036 の項目 (5)（start 冒頭でのリセット）も実装されると冗長になるが、defense in depth として両方残す。0031 単独で実装された場合、ストリーミング中のエラー後に stop→start すると stale な `stream_error=1` のため即座に `-5` を返すため、0031 側のリセットは必須

```c
void video_pipewire_session_stop(struct VideoSession* session) {
    if (!session || !atomic_load(&session->running)) {
        return;
    }
    atomic_store(&session->running, 0);
    pw_thread_loop_lock(session->thread_loop);
    if (session->stream) {
        pw_stream_disconnect(session->stream);
        spa_hook_remove(&session->stream_listener);
        pw_stream_destroy(session->stream);
        session->stream = NULL;
    }
    if (session->core) {
        spa_hook_remove(&session->core_listener);
        pw_core_disconnect(session->core);
        session->core = NULL;
    }
    atomic_store(&session->format_ready, 0);
    atomic_store(&session->stream_error, 0);
    pw_thread_loop_unlock(session->thread_loop);
    pw_thread_loop_stop(session->thread_loop);
}
```

### 競合解析

- `running=0` はロック取得前（:950 相当）に設定する。この瞬間に `on_process` が実行中でも :530 の `running` チェックで早期 return する。ロック取得待ちの間も `on_process` は `running=0` を見て安全に終了する
- `pw_stream_disconnect` は `state_changed(UNCONNECTED)` を同期的に発火する。`on_stream_state_changed`（:669-670）は `pw_thread_loop_signal` のみ呼び、`session->stream` へのアクセスはないため安全。続く `spa_hook_remove` 以降はコールバックは発火しない
- `negotiated_width` / `negotiated_height` / `negotiated_format` はリセットしない。`format_ready=0` により `on_process`（:534）が stale な値を使うことはないため
- コードスケッチの `if (session->stream)` / `if (session->core)` は防御的チェック。`running=1` で stop に入った場合、`running` は start 成功時（:939）にのみ 1 になるため非 NULL が構造的に保証されるが、防御的に残す

### destroy との整合性

stop で `stream = NULL` / `core = NULL` にするため、`video_pipewire_session_destroy`（:779, :783）の NULL チェックでスキップされ二重破棄は起きない。`session->context` と `session->thread_loop` は stop では意図的に破棄しない（再 start で再利用するため。destroy の :788-794 で破棄される）。

エラーパス・destroy のハードニングは 0036 で扱う。

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
