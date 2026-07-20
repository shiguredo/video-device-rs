# V4L2 の select() / FD_SET が FD_SETSIZE 超過時にスタックバッファオーバーフローを引き起こす

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-select-fd-setsize
- Polished: {YYYY-MM-DD}

## 目的

V4L2 バックエンドのキャプチャスレッド（`src/video_v4l2.c:542-550`）が `select()` / `FD_SET` を使用している。`fd >= FD_SETSIZE`（通常 1024）の場合、`FD_SET` は `fd_set` の固定長ビット配列の境界外に書き込み、スタックバッファオーバーフロー（メモリ破壊）を引き起こす。`poll()` に置き換えて修正する。

## 優先度根拠

- Medium。メモリ破壊に至るバグだが、発生条件が `fd >= 1024` に限定される
- 長時間稼働プロセスや多数の fd を開くプロセス（ブラウザ、メディアサーバ等）に組み込まれた場合、`/dev/video*` の fd が 1024 以上になり得る
- `poll()` への置き換えは容易で、副作用がない
- `/review-code` の致命的指摘として確認

## 現状

`src/video_v4l2.c:541-556`:

```c
while (atomic_load(&session->running)) {
    fd_set fds;
    FD_ZERO(&fds);
    FD_SET(session->fd, &fds);       // fd >= FD_SETSIZE で境界外書き込み

    struct timeval tv;
    tv.tv_sec = 1;
    tv.tv_usec = 0;

    int r = select(session->fd + 1, &fds, NULL, NULL, &tv);
    if (r < 0) {
        if (errno == EINTR) {
            continue;
        }
        break;
    }
    // ...
}
```

`fd_set` は固定長ビット配列（glibc では 1024 ビット）であり、`FD_SET` に境界チェックはない。`session->fd >= FD_SETSIZE` の場合、スタック上の `fd_set` の後方に書き込みが発生する。

## 設計方針

`select()` を `poll()` に置き換える。

1. `#include <poll.h>` を追加する
2. `fd_set` / `FD_ZERO` / `FD_SET` / `select` を `struct pollfd` / `poll` に置き換える
3. タイムアウトは `poll` の第 3 引数でミリ秒指定（1000ms）にする
4. `POLLERR` / `POLLHUP` / `POLLNVAL` のチェックを追加する（`select` の `r < 0` 相当）
5. `POLLIN` でデータ読み取り可能（`select` の `r > 0` 相当）

```c
struct pollfd pfd = { .fd = session->fd, .events = POLLIN };
int r = poll(&pfd, 1, 1000);
if (r < 0) {
    if (errno == EINTR) { continue; }
    break;
}
if (r == 0) { continue; }  // タイムアウト
if (pfd.revents & (POLLERR | POLLHUP | POLLNVAL)) { break; }
```

### 採用しない案

- `epoll` を使う: 単一 fd の監視には過剰。`poll` で十分
- `fd >= FD_SETSIZE` をチェックしてエラーにする: 根本解決にならない。デバイスが使用不能になるだけ

## 完了条件

- `capture_thread` の `select()` / `FD_SET` を `poll()` に置き換える
- `cargo build --workspace`（Linux、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
