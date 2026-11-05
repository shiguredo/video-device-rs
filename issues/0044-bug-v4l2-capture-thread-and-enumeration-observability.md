# V4L2 のキャプチャスレッド異常終了と列挙・設定の失敗が検知できない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-v4l2-capture-thread-observability
- Polished: 2026-11-05

## 目的

V4L2 バックエンドの失敗が利用者に通知されない箇所が複数ある。

1. `src/video_v4l2.c` の `capture_thread` は select 失敗、`VIDIOC_DQBUF` 失敗、不正な `buf.index`、`VIDIOC_QBUF` 失敗で `break` するが、`session->running` を 0 に戻さない。さらに `src/capture_ffi.rs` の `FfiCaptureImpl::start` は自身の `running` が真なら C の `video_v4l2_session_start` を呼ばずに `Ok(())` を返す。このためキャプチャスレッドが異常終了しても、再度 `start()` を呼ぶと成功扱いのままフレームが来ない状態を検知できない。デバイス切断などで発生する
2. `enumerate_device_formats` は `VIDIOC_ENUM_FRAMESIZES` が失敗したピクセルフォーマットを 1 件も追加しない。全フォーマットで失敗すると、キャプチャ可能なデバイスが `format_count == 0` で列挙される。`video_v4l2_enumerate_devices` は `enumerate_device_formats` の戻り値も確認していない
3. `VIDIOC_S_PARM` の戻り値を確認しておらず、フレームレート設定が拒否されても利用者に伝わらない
4. `VIDIOC_ENUM_FRAMEINTERVALS` の列挙で index 0 を二重に処理している
5. デバイス列挙が `/dev/video0` から `/dev/video63` 固定で、`/dev/video64`（65 台目）以降を取りこぼす

## 優先度根拠

- High。(1) は「start 済みなのにフレームが来ない」無音故障で、利用者が原因を特定できない
- (2) はキャプチャ可能なデバイスが「フォーマット 0 件」に見え、CI の device_info 検証も落ちる
- (3)(4)(5) は列挙結果の品質と可観測性の問題
- `/review-code` の重要指摘として確認済み

## 現状

`capture_thread` のループは `while (atomic_load(&session->running))` で、`break` 後に `running` を更新しない。`video_v4l2_session_start` は次のように早期 return する。

```c
if (atomic_load(&session->running)) {
    return 0;
}
```

ただし、`start()` の再呼び出しで `Ok(())` を返す直接の原因は Rust 側にある。`src/capture_ffi.rs` の `FfiCaptureImpl::start` は自身の `running` が真なら C を呼ばずに `Ok(())` を返す。

```rust
if self.running {
    return Ok(());
}
```

`enumerate_device_formats` は `VIDIOC_ENUM_FRAMESIZES` の初回 ioctl が失敗した場合にそのピクセルフォーマットのエントリを追加せず、`video_v4l2_enumerate_devices` 側でも戻り値を無視している。`VIDIOC_S_PARM` は `xioctl(fd, VIDIOC_S_PARM, &parm);` と戻り値を捨てている。`VIDIOC_ENUM_FRAMEINTERVALS` は index 0 の最初の呼び出しで discrete 値を処理した後、`frmival.index` を進めないまま同じ index 0 でループを再開するため、index 0 を二重に処理する。デバイス列挙は `for (int i = 0; i < 64; i++)` で `/dev/video0` から `/dev/video63` の 64 台だけを走査する。

## 設計方針

### 1. キャプチャスレッドの異常終了の通知

- `VideoSession` に `atomic_int faulted` と `int thread_started` を追加する
- `capture_thread` はエラーで `break` した場合に `faulted` を 1 にし、終端で `atomic_store(&session->running, 0)` を実行する。`stop` による正常終了（`running` が 0 にされたことによるループ終了）では `faulted` を立てない。エラー時は失敗した操作と、errno がある場合は errno を stderr に英語でログする
- `video_v4l2_session_stop` は `thread_started` が真なら `running` が 0 でも `pthread_join` し、join が成功したら `thread_started` を 0 にして `VIDIOC_STREAMOFF` を呼ぶ。`video_v4l2_session_destroy` は `video_v4l2_session_stop` を呼んでから mmap / fd / `VideoSession` を解放する
- `video_v4l2_session_start` は `faulted` が真なら失敗を返す。`faulted` は `stop` では解除しない（異常終了したセッションは再利用せず、新しい `VideoCapture` を構築し直す）
- Rust 側では `CaptureOps` に V4L2 用の faulted 照会関数（例: `video_v4l2_session_faulted`）を追加する。`FfiCaptureImpl::start` は自身の `running` 早期 return より前にこの照会を行い、真なら `self.running = false` にして `Error::CaptureFaulted` を返す。C の `video_v4l2_session_start` が失敗した場合も faulted を再確認し、真なら `Error::CaptureFaulted` を返す。これにより、`start()` の再呼び出しでも異常終了を検知できる。AVF / PipeWire は照会関数を持たない（`None`）ため挙動を変えない
- 非同期のエラーコールバックは追加しない。検知は次の `start()` が `Error::CaptureFaulted` を返すことで行う
- `Error::CaptureFaulted` の doc と `Display` を panic に限定しない表現に更新し、`README.md`、`src/lib.rs`、`src/capture.rs`、`src/capture_ffi.rs` の doc を V4L2 の挙動に合わせる

### 2. フォーマット列挙のフォールバック

- 各 `VIDIOC_ENUM_FMT` のピクセルフォーマットについて `VIDIOC_ENUM_FRAMESIZES` の初回呼び出しが失敗した場合、`VIDIOC_G_FMT` で現在の width / height を取得し、その `fmtdesc.pixelformat` を変換したピクセルフォーマットで 1 件のエントリを追加する。G_FMT の `pixelformat` は使わない（走査中のピクセルフォーマットと一致しないため）
- fps は既存の `VIDIOC_ENUM_FRAMEINTERVALS` 経路をそのまま使い、失敗時は既存の既定値（`min_fps = 1.0f`, `max_fps = 30.0f`）を維持する。`VIDIOC_G_FMT` も失敗した場合はエントリを追加せず、stderr に英語の警告を出す
- `enumerate_device_formats` が -1 を返した場合（open または realloc 失敗）、`video_v4l2_enumerate_devices` はそのデバイスを列挙結果に追加しない。`name` / `unique_id` / `device` を解放して `continue` し、stderr に英語の警告を出す。これにより `format_count == 0` のデバイスを追加しない

### 3. `VIDIOC_S_PARM` の失敗

- `VIDIOC_S_PARM` の戻り値を確認し、失敗したら要求 fps と errno を含む英語のログを stderr に出す
- セッション作成自体は継続する（fps 設定はキャプチャ開始の必須条件ではないため、現行の挙動を変えない）

### 4. `VIDIOC_ENUM_FRAMEINTERVALS` の二重処理

- index 0 から 1 回だけ走査するループに修正する。discrete の場合は全 index を走査して最小 / 最大 fps を求め、stepwise / continuous の場合は 1 回の呼び出しで min / max を取得する。既存の fps 計算式は維持する

### 5. デバイス列挙範囲

- `/sys/class/video4linux` のエントリを `opendir` / `readdir` で列挙し、`/dev/<name>` を開く。`opendir` に失敗した場合は従来の `/dev/video0` から `/dev/video63` の走査にフォールバックする
- `/sys/class/video4linux` を利用できる環境では固定上限を廃止し、65 台目以降も列挙する

## 完了条件

- キャプチャ中にデバイスを切断した場合、次の `start()` が `Error::CaptureFaulted` を返す（非同期のエラーコールバックは追加しない）
- キャプチャスレッド異常終了時に、失敗した操作と、errno がある場合は errno を含む英語のログが出る
- 異常終了後に `stop()` または `destroy` を呼んだ場合、`thread_started` に基づいて `pthread_join` が行われ、スレッドリソースが残らない（コードレビューで確認）
- `VIDIOC_ENUM_FRAMESIZES` を実装しないドライバでも、`VIDIOC_G_FMT` が成功する限り `formats()` が空にならない
- `enumerate_device_formats` が -1 を返したデバイスは `format_count == 0` のまま列挙されない
- `VIDIOC_S_PARM` 失敗時に要求 fps と errno を含む英語のログが出る
- `VIDIOC_ENUM_FRAMEINTERVALS` の列挙で index 0 が二重に処理されず、discrete の場合は全 index を 1 回ずつ走査する（コードレビューで確認）
- `/sys/class/video4linux` を利用できる環境で 65 台以上（`/dev/video64` 以降を含む）のビデオデバイスがあるとき、全台のキャプチャ可能なビデオデバイスが列挙される。`/sys/class/video4linux` を利用できない環境では従来の 64 台までの走査にフォールバックする
- `Error::CaptureFaulted` の `Display` と、`README.md`、`src/lib.rs`、`src/capture.rs`、`src/capture_ffi.rs` の doc が V4L2 の挙動と一致する
- Linux で `cargo clippy --workspace --all-targets -- -D warnings` と `cargo test --workspace` が通る

## 解決方法

{完了時に記入}
