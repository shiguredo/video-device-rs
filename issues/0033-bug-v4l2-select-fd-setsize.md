# V4L2 の select() / FD_SET が FD_SETSIZE 超過時にスタックバッファオーバーフローを引き起こす

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-select-fd-setsize
- Polished: 2026-07-21

## 目的

V4L2 バックエンドのキャプチャスレッド（`src/video_v4l2.c:541-556`）が `select()` / `FD_SET` を使用している。`fd >= FD_SETSIZE`（通常 1024）の場合、`FD_SET` は `fd_set` の固定長ビット配列の境界外に書き込み、スタックバッファオーバーフロー（メモリ破壊）を引き起こす。`poll()` に置き換えて修正する。

## 優先度根拠

- Medium。メモリ破壊に至るバグだが、発生条件が `fd >= 1024` に限定される
- 「致命的」は発火時の影響度（メモリ破壊）、「Medium」は発火確率（通常は fd < 1024）。影響度と発火確率は別の軸
- 長時間稼働プロセスや多数の fd を開くプロセス（ブラウザ、メディアサーバ等）に組み込まれた場合、`/dev/video*` の fd が 1024 以上になり得る
- `poll()` への置き換えは容易で、副作用がない（タイムアウト精度: 今回ちょうど 1000ms なので select の μs 精度と poll の ms 精度で差なし。シグナル処理: 両者とも EINTR を返す。select の fd_set/timeval 破壊的書き換え: 現コードは毎ループ再初期化しているので影響なし）
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
    // :563 以降（DQBUF 処理）は変更なし
}
```

`fd_set` は固定長ビット配列（glibc では 1024 ビット）であり、`FD_SET` に境界チェックはない。`session->fd >= FD_SETSIZE` の場合、スタック上の `fd_set` の後方に書き込みが発生する。

## 設計方針

`select()` を `poll()` に置き換える。

1. `#include <poll.h>` を追加する（include ブロック :3-15 の `<linux/videodev2.h>`（:5）の次、アルファベット順）。`<sys/time.h>`（:14）は `v4l2_buffer.timestamp`（:576-577）が `struct timeval` 型のため維持する
2. `fd_set` / `FD_ZERO` / `FD_SET` / `select` を `struct pollfd` / `poll` に置き換える。`struct pollfd pfd` は while ループ内に置き毎回初期化する（ループ外配置も検討したが、1 秒タイムアウトのループで毎回の構造体初期化は実測上無視でき、可読性を優先）
3. タイムアウトは `poll` の第 3 引数でミリ秒指定（1000ms）にする
4. `POLLERR` / `POLLHUP` / `POLLNVAL` のチェックを追加する（fd 上のエラー状態を検出する。`select` では read 時のエラーとして現れる状態）。`POLLHUP` は USB カメラの物理取り外し等で返り、この場合キャプチャの継続は不可能なため `break` でスレッドを終了する。`POLLNVAL` は通常のライフサイクル（destroy が stop → pthread_join → close(fd) の順）では発生しないが、異常系の防御としてチェックする
5. `POLLIN` でデータ読み取り可能（`select` の `r > 0` 相当）。エラーフラグを除外した後は `POLLIN` がセットされているため暗黙的に読み取り可能だが、防御的プログラミングとして `if (pfd.revents & POLLIN)` の明示チェックを追加する

```c
struct pollfd pfd = { .fd = session->fd, .events = POLLIN };
int r = poll(&pfd, 1, 1000);
if (r < 0) {
    if (errno == EINTR) { continue; }
    break;
}
if (r == 0) { continue; }  // タイムアウト
if (pfd.revents & (POLLERR | POLLHUP | POLLNVAL)) { break; }
if (!(pfd.revents & POLLIN)) { continue; }  // 防御的チェック
// 以降、既存の DQBUF / コールバック / requeue 処理（:563 以降）は変更なし
```

置換範囲は :542-561（`fd_set` 初期化からタイムアウトチェックまで）。:563 以降の DQBUF / コールバック / requeue 処理は変更なし。POLLERR / POLLIN チェックはタイムアウトチェックと DQBUF の間に挿入する。POLLERR と POLLIN が同時にセットされても POLLERR を優先して `break` する（エラー状態のデバイスからの DQBUF は不正データやエラーを返すため、キャプチャ継続は意味がない）。

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
