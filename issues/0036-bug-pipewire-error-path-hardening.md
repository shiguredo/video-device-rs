# PipeWire エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-pipewire-error-path-hardening
- Polished: 2026-07-21

## 目的

PipeWire バックエンド（`src/video_pipewire.c`）のエラーパスに 5 つの不備がある。(1) `spa_pod_builder_add_object` の NULL 戻り値が未確認、(2) ストリーム状態待ちとデバイス列挙にタイムアウトがなく永久ブロックし得る、(3) start のエラーパスで `core_listener` を remove せずに disconnect している、(4) start のエラーパスと destroy で `stream_listener` を remove せずに `pw_stream_destroy` を呼んでいる、(5) start 冒頭で `stream_error` / `format_ready` をリセットしていない。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパス・異常時の不備
- (1) は NULL デリファレンスでクラッシュしうる
- (2) は PipeWire デーモンのハング時に永久ブロックする
- (3) は PipeWire のフック契約違反
- (4) は (3) と同種のフック契約違反。0031 実装後は stop→destroy パスでも到達不能になる（start の全エラーパスが `session->stream = NULL` を設定するため）。destroy への追加は defense in depth
- (5) は正常系に影響するバグ。ストリーミング中のエラー後に stop→start すると stale な `stream_error=1` のため即座に `-5` を返す
- `/review-code` の重要指摘として確認

## 現状

### (1) spa_pod_builder_add_object の NULL 戻り値未確認（src/video_pipewire.c:884）

```c
params[0] = spa_pod_builder_add_object(&builder, ...);
```

`params_buffer[1024]` が不足した場合 NULL が返る。NULL のまま `pw_stream_connect(params, 1)` を呼ぶと NULL デリファレンスでクラッシュ。

### (2) タイムアウトなし（src/video_pipewire.c:916-937, 315）

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

### (3) core_listener 未 remove（src/video_pipewire.c:839-844, 855-860）

`pw_core_add_listener`（:830）後、props 確保失敗（:839-844）や stream 生成失敗（:855-860）のパスで `spa_hook_remove(&session->core_listener)` なしに `pw_core_disconnect` している。正常な destroy パス（:784）では remove してから disconnect している。

### (4) stream_listener 未 remove（src/video_pipewire.c:904-912, 927-934, 779-781）

start のエラーパス（:904-912, :927-934）で `pw_stream_add_listener`（:863-865）で登録した `stream_listener` を remove せずに `pw_stream_destroy` を呼んでいる。また `video_pipewire_session_destroy`（:779-781）も `spa_hook_remove(&session->stream_listener)` なしに `pw_stream_destroy` を呼んでいる。0031 から委譲された項目。

### (5) start 冒頭でのリセット缺失（src/video_pipewire.c:801-942）

start がエラーパスで失敗した場合（:666 で `stream_error` が 1 に設定され、:926 でそれを検出する）、`running` は 0 のままなので stop は早期 return し、`stream_error` はリセットされない。次回の start で即座に `-5` を返す可能性がある。start 冒頭で `stream_error` と `format_ready` をリセットすべき。0031 から委譲された項目。

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

ストリーム状態待ちにタイムアウトを追加する。`pw_thread_loop_wait` の代わりに `pw_thread_loop_timedwait` を使用する。PipeWire の `pw_thread_loop_timedwait` の timeout 引数はナノ秒（`int64_t`）であるため、10 秒を意図するなら `INT64_C(10) * SPA_NSEC_PER_SEC` を指定する。10 秒の根拠: PipeWire デーモンの起動遅延と V4L2 デバイスのフォーマット交渉時間を考慮した余裕値。

`pw_thread_loop_timedwait` はタイムアウト時に負値（`-ETIMEDOUT`）を返す。戻り値をチェックし、負値の場合はタイムアウトとしてクリーンアップ（:927-934 と同じパターン）後に `return -5` を返す。10 秒は while ループ 1 反復あたりのタイムアウトであり、`session_core_done`（:685-690）が signal して状態が STREAMING に到達すればループを抜ける。signal なしに 10 秒経過した場合は PipeWire デーモンの異常とみなしてエラーにする。

```c
// while (1) ループ内の pw_thread_loop_wait を置換
int wait_res = pw_thread_loop_timedwait(session->thread_loop,
                                         INT64_C(10) * SPA_NSEC_PER_SEC);
if (wait_res < 0) {
    // タイムアウト: :927-934 と同じクリーンアップ（(4) 適用済み前提）
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

デバイス列挙の `pw_main_loop_run` には、`spa_loop_add_timer`（`pw_main_loop_get_loop` で取得した `spa_loop` に対して呼ぶ）でタイマーソースを追加し、コールバック内で `pw_main_loop_quit` を呼ぶ。タイムアウト値は 5 秒（`INT64_C(5) * SPA_NSEC_PER_SEC`）。デバイス列挙はストリーム交渉より軽量なため短めに設定。タイマーソースはスタック確保で十分（`pw_main_loop_destroy`（:338）が関数 return 前に呼ばれるため、明示的な `spa_loop_remove_source` は不要）。`expires = INT64_C(5) * SPA_NSEC_PER_SEC`、`rate = 0` でワンショット。コールバックは `spa_source_func_t` シグネチャで、`source->data` 経由で `EnumerateContext` を取得する。

タイムアウト後の部分結果の扱い: `param_done=0` のノード（フォーマット未取得または一部取得）は、取得済みのフォーマットだけでデバイスに含める（フォーマット 0 件のデバイスも列挙に含め、既存の「デバイスなし」挙動と整合）。`PendingNode` の `node_proxy` は `spa_hook_remove` + `pw_proxy_destroy` で破棄する（既存の :318-328 の後処理と同じパターン）。

### (3) の修正

エラーパス（:839-844, :855-860）に `spa_hook_remove(&session->core_listener)` を追加する。:839-844 は props 確保失敗で stream がまだ生成されていない（:853-854 より前）ため、`spa_hook_remove(&session->stream_listener)` は不要。`spa_hook_remove(&session->core_listener)` だけで正しい。

### (4) の修正

start のエラーパス（:904-912, :927-934）と `video_pipewire_session_destroy`（:779-781）に `spa_hook_remove(&session->stream_listener)` を追加する。0031 実装後は stop が `session->stream = NULL` にするため、destroy の :779 `if (session->stream)` は常に false になり、destroy への追加は defense in depth として機能する。

フック remove の順序根拠: `spa_hook_remove` を `pw_stream_destroy` / `pw_core_disconnect` より前に呼ぶのは、destroy/disconnect 中にコールバックが発火するのを防ぐため。既存の destroy パス（:784: `spa_hook_remove` → `pw_core_disconnect`）と同じパターン。

### (5) の修正

`video_pipewire_session_start` の冒頭（`running` チェック後、`thread_loop_start` 前）で `atomic_store(&session->format_ready, 0)` と `atomic_store(&session->stream_error, 0)` をリセットする。`running` チェック（:807-809）で `running=1` のときは `return 0` するためリセットは実行されず、動作中のストリームの状態を破壊しない。`running=0` のときだけリセットが実行される。

### 実装順序

(1) のコード例には `spa_hook_remove(&session->stream_listener)` が含まれており、これは (4) の修正内容そのもの。(4) を先に適用するか、(1) と (4) を同時に適用すること。

## 完了条件

- (1) `spa_pod_builder_add_object` の NULL チェックを追加し、エラーパスで stream/core を正しくクリーンアップする
- (2) ストリーム状態待ちに `pw_thread_loop_timedwait` でタイムアウト（10 秒）を追加する。デバイス列挙に `spa_loop_add_timer` でタイムアウト（5 秒）を追加する
- (3) start のエラーパス（:839-844, :855-860）で `spa_hook_remove(&session->core_listener)` を呼ぶ
- (4) start のエラーパス（:904-912, :927-934）と destroy（:779-781）で `spa_hook_remove(&session->stream_listener)` を呼ぶ
- (5) start 冒頭で `stream_error` と `format_ready` をリセットする
- `cargo build --workspace --features pipewire`（Linux）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
