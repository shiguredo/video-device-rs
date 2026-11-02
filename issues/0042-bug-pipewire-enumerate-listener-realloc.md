# PipeWire のデバイス列挙で PendingNode 配列の realloc が登録済みリスナーを無効化する

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-enumerate-listener-realloc
- Polished: {YYYY-MM-DD}

## 目的

`src/video_pipewire.c` の `video_pipewire_enumerate_devices` / `enum_registry_global` は、Video/Source ノードを 1 台見つけるたびに `ctx->pending` を `realloc` して `PendingNode` 配列を拡張し、その要素の `node_listener` と `p` を `pw_node_add_listener` に渡している。

`realloc` で配列が移動すると、PipeWire の proxy 側に登録済みの `spa_hook` と `data`（`PendingNode*`）が解放済みの旧領域を指したままになる。9 台目以降の Video/Source が存在する環境では、以降の `on_node_param` が解放済みメモリへ書き込み、列挙終了時の `spa_hook_remove` / `pw_proxy_destroy` も旧 link を辿るため、ヒープ破壊（use-after-free）に至る。

## 優先度根拠

- High。メモリ破壊であり、仮想カメラや複数カメラを接続した開発機で現実に発生しうる
- 8 台以下では `realloc` が起きないため、通常の CI では検出されない
- `/review-code` の致命的指摘として 3 周にわたりコードを再確認済み

## 現状

`enum_registry_global` は `ctx->count >= ctx->capacity` のとき `ctx->devices` と `ctx->pending` を `realloc` する（初回 capacity 8）。その後 `struct PendingNode* p = &ctx->pending[ctx->count];` を取得し、次のように登録する。

```c
pw_node_add_listener((struct pw_node*)p->node_proxy, &p->node_listener, &enum_node_events, p);
```

`PendingNode` は配列内に値で埋め込まれており、`spa_hook` も構造体の一部である。9 台目の global で配列が移動すると、1〜8 台目の `p` と `node_listener` のアドレスが変わる。PipeWire は登録時のアドレスを保持するため、旧アドレスへの書き込み・解放が発生する。

列挙終了後の後始末も移動後の `p` に対して `spa_hook_remove` / `pw_proxy_destroy` を呼ぶため、hook の `link.prev` / `link.next` が旧領域を指したままになり、param イベントの到着順に関係なく破壊が起きる。

## 設計方針

`PendingNode` のアドレスを固定する。具体的には次のいずれか。

- `PendingNode*` の配列にして各要素を個別に `calloc` する
- 全 global をいったん収集し、配列を確定させてから一括で listener 登録する

あわせて、後始末は `spa_hook_remove` → `pw_proxy_destroy` の順を守り、確保済みの `formats` を解放する。

## 完了条件

- 9 台以上の Video/Source が存在する環境（または 9 ノードを模したテスト）で、列挙がクラッシュせず全デバイスとフォーマットが取得できる
- AddressSanitizer 有効ビルドで use-after-free が検出されない
- 列挙終了後に listener / hook / proxy の解放漏れがない
- 8 台以下の既存挙動が変わらない

## 解決方法

{完了時に記入}
