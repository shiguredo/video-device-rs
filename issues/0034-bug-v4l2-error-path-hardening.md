# V4L2 エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-v4l2-error-path-hardening
- Polished: {YYYY-MM-DD}

## 目的

V4L2 バックエンド（`src/video_v4l2.c`）のエラーパスに 3 つの不備がある。(1) `V4L2_CAP_DEVICE_CAPS` 未確認で `device_caps` を参照している、(2) `VIDIOC_REQBUFS` で確保したカーネルバッファがエラーパスで解放されない、(3) QBUF 部分失敗でセッションが回復不能になる。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパスの不備であり、通常パスでは発生しない
- (1) は古いドライバでデバイス列挙が正しく動作しない可能性がある
- (2) はカーネルリソースのリーク
- (3) はエラー発生後にセッションの破棄・再作成が必要になる
- `/review-code` の重要指摘として確認

## 現状

### (1) V4L2_CAP_DEVICE_CAPS 未確認（src/video_v4l2.c:264）

```c
if (!(cap.device_caps & V4L2_CAP_VIDEO_CAPTURE)) {
```

V4L2 仕様では `cap.capabilities & V4L2_CAP_DEVICE_CAPS` がセットされている場合のみ `device_caps` が有効。未セットのドライバでは `device_caps` は不定値であり、正しいデバイスが列挙から漏れる。

### (2) REQBUFS で確保したカーネルバッファがエラーパスで解放されない（src/video_v4l2.c:380-428）

`init_mmap` 内で `VIDIOC_REQBUFS` が成功した後、`calloc` 失敗（:396）、`QUERYBUF` 失敗（:409）、`mmap` 失敗（:417）のいずれでも `goto fail` → `cleanup_mmap` に至るが、`cleanup_mmap`（:367-378）は `munmap` + `free` のみで `VIDIOC_REQBUFS(count=0)` によるカーネル側バッファ解放を行わない。

### (3) QBUF 部分失敗でセッションが回復不能に（src/video_v4l2.c:669-685）

`video_v4l2_session_start` の QBUF ループで i 番目の QBUF が失敗すると -1 を返すが、0..i-1 は既にドライバのキューに入っている。STREAMOFF も dequeue も行われない。再 start 時に同じバッファを QBUF すると `EINVAL` となり、destroy するまでセッションを再利用できない。

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

`cleanup_mmap` に `VIDIOC_REQBUFS(count=0)` によるカーネル側バッファ解放を追加する。`session->fd` が必要なので、`cleanup_mmap` の引数に `fd` を追加するか、`session` を渡す。

### (3) の修正

QBUF ループ失敗時に `VIDIOC_STREAMOFF` を呼んでキューをリセットしてから -1 を返す。または、QBUF 前に全バッファを dequeue してから再 QBUF する。

## 完了条件

- (1) `V4L2_CAP_DEVICE_CAPS` を確認してから `device_caps` を参照する
- (2) `cleanup_mmap` で `VIDIOC_REQBUFS(count=0)` を呼びカーネルバッファを解放する
- (3) QBUF 部分失敗時に STREAMOFF でキューをリセットする
- `cargo build --workspace`（Linux、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
