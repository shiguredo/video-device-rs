# V4L2 エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-error-path-hardening
- Polished: 2026-07-21

## 目的

V4L2 バックエンド（`src/video_v4l2.c`）の API 使用とエラーパスに 3 つの不備がある。(1) `V4L2_CAP_DEVICE_CAPS` 未確認で `device_caps` を参照している（API 使用の互換性バグ）、(2) `VIDIOC_REQBUFS` で確保したカーネルバッファがエラーパスで解放されない、(3) QBUF 部分失敗でセッションが回復不能になる。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパスの不備であり、通常パスでは発生しない
- (1) は古いドライバでデバイス列挙が正しく動作しない可能性がある
- (2) はカーネルリソースのリーク
- (3) はエラー発生後にセッションの破棄・再作成が必要になる
- `/review-code` の重要指摘として確認

## 現状

### (1) V4L2_CAP_DEVICE_CAPS 未確認（src/video_v4l2.c:264, :270, :446-447）

```c
// :264（video_v4l2_enumerate_devices）
if (!(cap.device_caps & V4L2_CAP_VIDEO_CAPTURE)) {

// :270（video_v4l2_enumerate_devices）
if (!(cap.device_caps & V4L2_CAP_STREAMING)) {

// :446-447（video_v4l2_session_create）
if (!(cap.device_caps & V4L2_CAP_VIDEO_CAPTURE) ||
    !(cap.device_caps & V4L2_CAP_STREAMING)) {
```

V4L2 仕様では `cap.capabilities & V4L2_CAP_DEVICE_CAPS` がセットされている場合のみ `device_caps` が有効。未セットのドライバでは `device_caps` は不定値であり、正しいデバイスが列挙から漏れる。修正は 3 箇所（:264, :270, :446-447）に適用する。

後方互換性: V4L2 仕様上 `capabilities` は全キャップビットのスーパーセットであるため、この修正は偽陰性（列挙漏れ）を減らすのみで、偽陽性（誤列挙）を導入しない。

### (2) REQBUFS で確保したカーネルバッファのエラーパス処理（src/video_v4l2.c:380-428）

`init_mmap` 内で `VIDIOC_REQBUFS` が成功した後、`QUERYBUF` 失敗（:409）、`mmap` 失敗（:417）は `goto fail` → `cleanup_mmap` に至るが、`cleanup_mmap`（:367-378）は `munmap` + `free` のみで `VIDIOC_REQBUFS(count=0)` によるカーネル側バッファ解放を行わない。

なお、`calloc` 失敗（:396）と `req.count < 2`（:391-393）は `goto fail` を通らず直接 `return -1` する。これらのパスも REQBUFS 成功後のため、カーネルバッファが残る。

ただし、`init_mmap` 失敗後 `video_v4l2_session_create`（:511-514）は即座に `close(fd)` を呼ぶため、V4L2 の fd クローズ時自動解放により実際にはリークは発生しない。本修正は防御的硬化であり、`close(fd)` に依存しない明示的なリソース解放を目的とする。

closed/0010 はユーザ空間の mmap/buffers リークを修正済み。本 issue はカーネル側 REQBUFS バッファの明示的解放を扱う。

### (3) QBUF 部分失敗でセッションが回復不能に（src/video_v4l2.c:669-685）

`video_v4l2_session_start` の QBUF ループで i 番目の QBUF が失敗すると -1 を返すが、0..i-1 は既にドライバのキューに入っている。STREAMOFF も dequeue も行われない。再 start 時に同じバッファを QBUF するとエラー（ドライバ依存で `EINVAL` や `EBUSY`）となり、destroy するまでセッションを再利用できない。

また、QBUF ループが全件成功した後に STREAMON が失敗した場合（:683-684）も、全バッファがキューに残ったまま `return -1` する。このパスもキューリセットが必要。

## 設計方針

### (1) の修正

```c
uint32_t caps = (cap.capabilities & V4L2_CAP_DEVICE_CAPS)
                    ? cap.device_caps
                    : cap.capabilities;
if (!(caps & V4L2_CAP_VIDEO_CAPTURE)) { ... }
if (!(caps & V4L2_CAP_STREAMING)) { ... }
```

### (2) の修正

`cleanup_mmap`（:367）は既に `struct VideoSession* session` を引数に取っており、`session->fd` にアクセス可能。`cleanup_mmap` 内で munmap ループの後、`free(session->buffers)` の前に `session->fd` を使って `VIDIOC_REQBUFS(count=0)` を呼ぶ。

`goto fail` を通らないパス（`req.count < 2`、`calloc` 失敗）も REQBUFS 成功後のため、これらのパスにも REQBUFS(count=0) を追加する。具体的には `req.count < 2` の `return -1` 前と `calloc` 失敗の `return -1` 前に `VIDIOC_REQBUFS(count=0)` を呼ぶ。

### (3) の修正

QBUF ループ失敗時と STREAMON 失敗時に `VIDIOC_STREAMOFF` を呼んでキューをリセットしてから -1 を返す。V4L2 の `VIDIOC_STREAMOFF` はストリーミング状態に関わらずキュー上の全バッファを dequeue するため、STREAMON 前でも安全に呼び出せる。

実装では `enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;` を QBUF ループより前に宣言し、QBUF 失敗時と STREAMON 失敗時の両方で使用する。

```c
// QBUF ループ失敗時（:676-678 相当）
if (xioctl(session->fd, VIDIOC_QBUF, &buf) < 0) {
    xioctl(session->fd, VIDIOC_STREAMOFF, &type);
    return -1;
}

// STREAMON 失敗時（:683-684 相当）
if (xioctl(session->fd, VIDIOC_STREAMON, &type) < 0) {
    xioctl(session->fd, VIDIOC_STREAMOFF, &type);
    return -1;
}
```

## 完了条件

- (1) `V4L2_CAP_DEVICE_CAPS` を確認してから `device_caps` を参照する（`:264`、`:270`、`:446-447` の 3 箇所）
- (2) `cleanup_mmap` で `VIDIOC_REQBUFS(count=0)` を呼びカーネルバッファを解放する。`goto fail` を通らないパス（`req.count < 2`、`calloc` 失敗）にも REQBUFS(count=0) を追加する
- (3) QBUF 部分失敗時と STREAMON 失敗時に STREAMOFF でキューをリセットする
- `cargo build --workspace`（Linux、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
