# V4L2 のキャプチャスレッド異常終了と列挙・設定の失敗が検知できない

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-v4l2-capture-thread-observability
- Polished: {YYYY-MM-DD}

## 目的

V4L2 バックエンドの失敗が利用者に通知されない箇所が複数ある。

1. `src/video_v4l2.c` の `capture_thread` は select 失敗、`VIDIOC_DQBUF` 失敗、不正な `buf.index`、`VIDIOC_QBUF` 失敗で `break` するが、`session->running` を 0 に戻さない。`video_v4l2_session_start` は `running` が真なら 0 を返すため、Rust 側の `VideoCapture::start` も `Ok(())` を返し続け、フレームが来ない状態を検知できない。デバイス切断などで発生する
2. `enumerate_device_formats` は `VIDIOC_ENUM_FRAMESIZES` が失敗したピクセルフォーマットを 1 件も追加しない。全フォーマットで失敗すると、キャプチャ可能なデバイスが `format_count == 0` で列挙される。戻り値も確認していない
3. `VIDIOC_S_PARM` の戻り値を確認しておらず、フレームレート設定が拒否されても利用者に伝わらない
4. `VIDIOC_ENUM_FRAMEINTERVALS` の列挙で index 0 を二重に処理している
5. デバイス列挙が `/dev/video0` から `/dev/video63` 固定で、64 台目以降を取りこぼす

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

`enumerate_device_formats` は `VIDIOC_ENUM_FRAMESIZES` の初回 ioctl が失敗した場合にそのピクセルフォーマットのエントリを追加せず、`video_v4l2_enumerate_devices` 側でも戻り値を無視している。

## 設計方針

- スレッド終了時に必ず `session->running` を 0 にする（スレッド関数の終端で `atomic_store(&session->running, 0)`）
- スレッドが異常終了したことを Rust 側へ伝える状態（`faulted` フラグなど）を追加し、次の `start()` で `Error::CaptureFaulted` 相当を返すか再作成する
- `VIDIOC_ENUM_FRAMESIZES` 失敗時は `VIDIOC_G_FMT` の現在値を 1 件フォールバックとして登録する
- `VIDIOC_S_PARM` の失敗を検知し、少なくとも stderr へ英語のログを出す
- `FRAMEINTERVALS` の index 0 二重処理を解消する
- 列挙範囲を `/sys/class/video4linux` ベースにするか、上限を引き上げる

## 完了条件

- キャプチャ中にデバイスを切断した場合、フレーム停止が `start()` の失敗またはエラー通知として観測できる
- `VIDIOC_ENUM_FRAMESIZES` を実装しないドライバでも `formats()` が空にならない
- `VIDIOC_S_PARM` 失敗時にログが出る
- 64 台以上のビデオデバイスがある環境で全台が列挙される
- `cargo clippy --workspace --all-targets -- -D warnings` と `cargo test --workspace` が通る

## 解決方法

{完了時に記入}
