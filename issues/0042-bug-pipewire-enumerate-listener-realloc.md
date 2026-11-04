# PipeWire のデバイス列挙で PendingNode 配列の realloc が登録済みリスナーを無効化する

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-enumerate-listener-realloc
- Polished: 2026-11-04

## 目的

`src/video_pipewire.c` の `video_pipewire_enumerate_devices` が呼ぶ `enum_registry_global` は、Video/Source ノードを見つけると、`ctx->count >= ctx->capacity` のとき `ctx->pending` を `realloc` して `PendingNode` 配列を拡張する。その後、`ctx->pending[ctx->count]` の `node_listener` と `p` を `pw_node_add_listener` に渡して登録し、`ctx->count` を増やす。

`realloc` で配列が移動すると、PipeWire の proxy 側のリスナーリストは登録時の `spa_hook` を指したままになり、そのフックの `data`（`PendingNode*`）も解放済みの旧領域を指したままになる。9 台目以降の Video/Source が存在する環境で realloc 後に param イベントが届くと、`on_node_param` が解放済みメモリへ書き込むヒープ破壊（use-after-free）に至る。列挙終了時の `spa_hook_remove` は移動後のフックの `link` を使ってリストから外すため後始末自体は成立するが、それまでの間は proxy のリスナーリストが旧フックを指したままである。

## 優先度根拠

- High。メモリ破壊であり、仮想カメラや複数カメラを接続した開発機で現実に発生しうる
- 8 台以下では既存要素の移動を伴う `realloc` が起きないため、通常の CI では検出されない
- `/review-code` の致命的指摘として 3 周にわたりコードを再確認済み

## 現状

`enum_registry_global` は `ctx->count >= ctx->capacity` のとき `ctx->devices` と `ctx->pending` を `realloc` する（初回 capacity 8）。その後 `struct PendingNode* p = &ctx->pending[ctx->count];` を取得し、次のように登録する。

```c
pw_node_add_listener((struct pw_node*)p->node_proxy, &p->node_listener, &enum_node_events, p);
```

`PendingNode` は配列内に値で埋め込まれており、`spa_hook` も構造体の一部である。9 台目の global で配列が移動すると、1〜8 台目の `p` と `node_listener` のアドレスが変わる。PipeWire は登録時のアドレスを保持するため、旧アドレスへの書き込み・解放が発生する。

列挙終了後の後始末は移動後の `p` に対して `spa_hook_remove` / `pw_proxy_destroy` を呼ぶ。`spa_hook_remove` は移動後のフックの `link` を使ってリストから外すため、後始末自体は成立する。しかし、それまでの間は proxy 側のリスナーリストが旧フックを指したままであり、realloc 後に param イベントが届くと旧 `PendingNode` への書き込みが起きる。

## 設計方針

`PendingNode` のアドレスを固定する。具体的には次のいずれか。

- `PendingNode*` の配列にして各要素を個別に `calloc` する
- 全 global をいったん収集し、配列を確定させてから一括で listener 登録する

第一案（個別 `calloc`）を採用する。第二案は global の収集状態を追加で持つ必要があり、listener 登録を 1 段目 sync 完了後へ移す組み替えを伴うため、変更範囲が大きい。

あわせて、後始末は `spa_hook_remove` → `pw_proxy_destroy` の順を守る。`PendingNode` の `formats` は `VideoDevice` へ移譲し、移譲しなかった分だけ解放する。第一案のように `PendingNode` を個別に確保した場合は、各 `PendingNode` と `ctx->pending` 配列も解放する。

## 完了条件

- 9 台以上の Video/Source が存在する環境、または PipeWire 上に 9 個の Video/Source ノードを用意するテストで、列挙がクラッシュせず全デバイスとフォーマットが取得できる
- AddressSanitizer 有効ビルドで use-after-free が検出されない
- 列挙終了後に listener / hook / proxy / `PendingNode` / `ctx->pending` 配列の解放漏れがない
- 8 台以下の既存挙動が変わらない

## 解決方法

{完了時に記入}
