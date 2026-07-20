# PipeWire エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-pipewire-error-path-hardening
- Polished: {YYYY-MM-DD}

## 目的

PipeWire バックエンド（`src/video_pipewire.c`）のエラーパスに 3 つの不備がある。(1) `spa_pod_builder_add_object` の NULL 戻り値が未確認、(2) ストリーム状態待ちとデバイス列挙にタイムアウトがなく永久ブロックし得る、(3) start のエラーパスで `core_listener` を remove せずに disconnect している。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパス・異常時の不備
- (1) は NULL デリファレンスでクラッシュしうる
- (2) は PipeWire デーモンのハング時に永久ブロックする
- (3) は PipeWire のフック契約違反
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

## 設計方針

### (1) の修正

`spa_pod_builder_add_object` の戻り値が NULL の場合、エラーを返す。

```c
params[0] = spa_pod_builder_add_object(&builder, ...);
if (!params[0]) {
    // エラーパス: stream/core をクリーンアップして return -5
}
```

### (2) の修正

ストリーム状態待ちにタイムアウト（例: 10 秒）を追加する。`pw_thread_loop_wait` の代わりに `pw_thread_loop_timed_wait` を使用する。

デバイス列挙の `pw_main_loop_run` には、`pw_loop_add_idle` またはタイマーでタイムアウトを実装する。

### (3) の修正

エラーパスに `spa_hook_remove(&session->core_listener)` を追加する。

## 完了条件

- (1) `spa_pod_builder_add_object` の NULL チェックを追加する
- (2) ストリーム状態待ちとデバイス列挙にタイムアウトを追加する
- (3) エラーパスで `spa_hook_remove` を呼ぶ
- `cargo build --workspace --features pipewire`（Linux）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
