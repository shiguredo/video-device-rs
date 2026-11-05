# PipeWire エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-pipewire-error-path-hardening
- Polished: 2026-11-04

## 目的

PipeWire バックエンド（`src/video_pipewire.c`）のエラーパスに 5 つの不備がある。(1) `spa_pod_builder_add_object` の NULL 戻り値が未確認、(2) ストリーム状態待ちとデバイス列挙にタイムアウトがなく永久ブロックし得る、(3) start のエラーパスで `core_listener` を remove せずに disconnect している、(4) start のエラーパスと destroy で `stream_listener` を remove せずに `pw_stream_destroy` を呼んでいる、(5) start 冒頭で `stream_error` / `format_ready` をリセットしていない。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパス・異常時の不備
- (1) は NULL の場合にフォーマット未指定のまま接続が進み、エラーが遅れて表面化する
- (2) は PipeWire デーモンのハング時に永久ブロックする
- (3) は PipeWire のフック契約違反
- (4) は (3) と同種のフック契約違反。0031 実装後は destroy の stream 破棄は到達不能になる（0031 の stop が `session->stream = NULL` を設定し、start の全エラーパスも `session->stream = NULL` を設定するため）。destroy への追加は defense in depth
- (5) は正常系に影響するバグ。ストリーミング中のエラー後に stop→start すると stale な `stream_error=1` のため即座に `-5` を返す
- `/review-code` の重要指摘として確認

## 現状

### (1) spa_pod_builder_add_object の NULL 戻り値未確認（`video_pipewire_session_start`）

```c
params[0] = spa_pod_builder_add_object(&builder, ...);
```

`params_buffer[1024]` が不足した場合 `spa_pod_builder_add_object` は NULL を返す。NULL を `pw_stream_connect` に渡しても PipeWire 側は NULL パラメータをスキップするためクラッシュはしないが、EnumFormat が登録されずフォーマット未指定のまま接続が進み、エラーが遅れて表面化する。NULL を検出して明示的にエラーを返す。

### (2) タイムアウトなし（`video_pipewire_session_start` のストリーム状態待ち、`video_pipewire_enumerate_devices` のデバイス列挙）

ストリーム状態待ち:

```c
while (1) {
    // ...
    pw_thread_loop_wait(session->thread_loop);  // 永久ブロックし得る
}
```

デバイス列挙:

```c
pw_main_loop_run(ctx.loop);  // pw_main_loop_quit まで戻らない
```

PipeWire デーモンが応答しない場合、いずれも永久にブロックする。

### (3) core_listener 未 remove（`video_pipewire_session_start` の props 確保失敗パスと stream 生成失敗パス）

`pw_core_add_listener` 後、props 確保失敗や stream 生成失敗のパスで `spa_hook_remove(&session->core_listener)` なしに `pw_core_disconnect` している。正常な `video_pipewire_session_destroy` では remove してから disconnect している。

### (4) stream_listener 未 remove（`video_pipewire_session_start` のエラーパスと `video_pipewire_session_destroy`）

start のエラーパスで `pw_stream_add_listener` で登録した `stream_listener` を remove せずに `pw_stream_destroy` を呼んでいる。また `video_pipewire_session_destroy` も `spa_hook_remove(&session->stream_listener)` なしに `pw_stream_destroy` を呼んでいる。0031 から委譲された項目。

### (5) start 冒頭でのリセット漏れ（`video_pipewire_session_start`）

start がエラーパスで失敗した場合（`on_stream_state_changed` で `stream_error` が 1 に設定され、start の状態待ちループでそれを検出する）、`running` は 0 のままなので stop は早期 return し、`stream_error` はリセットされない。次回の start で即座に `-5` を返す可能性がある。start 冒頭で `stream_error` と `format_ready` をリセットすべき。0031 から委譲された項目。

## 設計方針

### (1) の修正

`spa_pod_builder_add_object` の戻り値が NULL の場合、エラーを返す。クリーンアップは以下の順序で行う:

```c
params[0] = spa_pod_builder_add_object(&builder, ...);
if (!params[0]) {
    spa_hook_remove(&session->stream_listener);
    pw_stream_destroy(session->stream);
    session->stream = NULL;
    spa_hook_remove(&session->core_listener);
    pw_core_disconnect(session->core);
    session->core = NULL;
    pw_thread_loop_unlock(session->thread_loop);
    pw_thread_loop_stop(session->thread_loop);
    return -5;
}
```

### (2) の修正

ストリーム状態待ちにタイムアウトを追加する。`pw_thread_loop_wait` の代わりに `pw_thread_loop_timed_wait` を使用する。`pw_thread_loop_timed_wait` の timeout 引数は秒（`int`）であるため、10 秒を意図するなら `10` を指定する。10 秒の根拠: PipeWire デーモンの起動遅延と V4L2 デバイスのフォーマット交渉時間を考慮した余裕値。

`pw_thread_loop_timed_wait` はタイムアウト時に正の `ETIMEDOUT` を返し、その他のエラーは負の errno を返す。戻り値が 0 以外の場合はタイムアウトまたはエラーとしてクリーンアップ（(4) 適用後の start エラーパスと同じパターン）後に `return -5` を返す。10 秒は while ループ 1 反復あたりのタイムアウトであり、`session_core_done` が signal して状態が STREAMING に到達すればループを抜ける。signal なしに 10 秒経過した場合は PipeWire デーモンの異常とみなしてエラーにする。シグナルが来続ける限り待機は延長されるため、このタイムアウトは「無信号状態の検出」であり、開始からの絶対時間の上限ではない。

```c
// while (1) ループ内の pw_thread_loop_wait を置換
int wait_res = pw_thread_loop_timed_wait(session->thread_loop, 10);
if (wait_res != 0) {
    // タイムアウトまたはエラー: (4) 適用後の start エラーパスと同じクリーンアップ
    spa_hook_remove(&session->stream_listener);
    pw_stream_destroy(session->stream);
    session->stream = NULL;
    spa_hook_remove(&session->core_listener);
    pw_core_disconnect(session->core);
    session->core = NULL;
    pw_thread_loop_unlock(session->thread_loop);
    pw_thread_loop_stop(session->thread_loop);
    return -5;
}
```

デバイス列挙の `pw_main_loop_run` には、`pw_loop_add_timer(pw_main_loop_get_loop(ctx.loop), on_enum_timeout, &ctx)` でタイマーソースを追加し、`pw_loop_update_timer` で `struct timespec value = {5, 0}`、`interval = {0, 0}`、`absolute = false` のワンショットを設定する。タイムアウト値は 5 秒。デバイス列挙はストリーム交渉より軽量なため短めに設定。コールバックは `spa_source_timer_func_t`（`void (*)(void *data, uint64_t expirations)`）で、`data` 引数に `EnumerateContext*` を渡す。コールバック内で `pw_main_loop_quit(ctx->loop)` を呼ぶ。

`pw_loop_add_timer` はループ utils がソースを確保する。`pw_main_loop_destroy` が関数 return 前に呼ばれるため、明示的な `pw_loop_destroy_source` は不要（呼んでもよい）。`pw_loop_add_timer` が NULL を返した場合は `pw_loop_update_timer` を呼ばず、タイムアウトなしで列挙を続行する（既存挙動へのフォールバック）。`pw_loop_update_timer` の戻り値も確認し、失敗した場合はタイムアウトなしで列挙を続行する。

タイムアウト後の部分結果の扱い: `param_done=0` のノード（フォーマット未取得または一部取得）は、取得済みのフォーマットだけでデバイスに含める（フォーマット 0 件のデバイスも列挙に含め、既存の「デバイスなし」挙動と整合）。`PendingNode` の `node_proxy` は `spa_hook_remove` + `pw_proxy_destroy` で破棄する（既存の `video_pipewire_enumerate_devices` の後処理ループと同じパターン）。

### (3) の修正

エラーパス（props 確保失敗、stream 生成失敗）に `spa_hook_remove(&session->core_listener)` を追加する。props 確保失敗は stream がまだ生成されていないため、`spa_hook_remove(&session->stream_listener)` は不要。`spa_hook_remove(&session->core_listener)` だけで正しい。

### (4) の修正

start のエラーパスと `video_pipewire_session_destroy` に `spa_hook_remove(&session->stream_listener)` を追加する。0031 実装後は stop が `session->stream = NULL` にするため、destroy の `if (session->stream)` は常に false になり、destroy への追加は defense in depth として機能する。

フック remove の順序根拠: `spa_hook_remove` を `pw_stream_destroy` / `pw_core_disconnect` より前に呼ぶのは、destroy/disconnect 中にコールバックが発火するのを防ぐため。既存の `video_pipewire_session_destroy` の core 破棄パス（`spa_hook_remove(&session->core_listener)` → `pw_core_disconnect`）と同じパターン。

### (5) の修正

`video_pipewire_session_start` の冒頭（`running` チェック後、`thread_loop_start` 前）で `atomic_store(&session->format_ready, 0)` と `atomic_store(&session->stream_error, 0)` をリセットする。`running` チェックで `running=1` のときは `return 0` するためリセットは実行されず、動作中のストリームの状態を破壊しない。`running=0` のときだけリセットが実行される。

### 実装順序

(1) のコード例には `spa_hook_remove(&session->stream_listener)` が含まれており、これは (4) の修正内容そのもの。(4) を先に適用するか、(1) と (4) を同時に適用すること。

## 完了条件

- (1) `spa_pod_builder_add_object` の NULL チェックを追加し、エラーパスで stream/core を正しくクリーンアップする
- (2) ストリーム状態待ちに `pw_thread_loop_timed_wait` でタイムアウト（10 秒）を追加する。デバイス列挙に `pw_loop_add_timer` / `pw_loop_update_timer` でタイムアウト（5 秒）を追加する
- (3) start のエラーパス（props 確保失敗、stream 生成失敗）で `spa_hook_remove(&session->core_listener)` を呼ぶ
- (4) start のエラーパスと destroy で `spa_hook_remove(&session->stream_listener)` を呼ぶ
- (5) start 冒頭で `stream_error` と `format_ready` をリセットする
- `cargo build --workspace --features pipewire`（Linux）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
