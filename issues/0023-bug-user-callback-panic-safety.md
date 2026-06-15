# ユーザフレームコールバックの panic を catch_unwind で吸収していない

- Priority: High
- Created: 2026-06-15
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/fix-user-callback-panic-safety
- Polished: 2026-06-15

## 目的

`src/capture_ffi.rs::frame_callback` および `src/capture_mf.rs::process_sample` が呼び出すユーザフレームコールバックを `std::panic::catch_unwind` で囲っていない。ユーザコールバックが panic すると、

- `frame_callback` 経路 (FFI 境界): C 側スレッド (V4L2 の `pthread`、AVF の dispatch queue、PipeWire の `pw_thread_loop` の event スレッド) から呼ばれた `extern "C" fn` の unwind は `abort` を引き起こし、C 側のフレーム後処理 (バッファ返却) が走らずバッファプールが枯渇する
- `process_sample` 経路 (Windows MF): Rust の `thread::spawn` 経由なので abort にはならないが、`JoinHandle::join()` が `Err` を返し、`MfCaptureImpl::stop` 側で callback が永久消失する (0022 が応急処置として `eprintln!` 可視化を行うが、本質的には panic を抑止すべき)

本 issue では両経路のユーザコールバック呼び出し箇所を `catch_unwind(AssertUnwindSafe(...))` で包み、panic を吸収して通常リターンする。

## 優先度根拠

- High。Rust 2024 edition では `extern "C"` 境界を越える unwind は `abort` になり、プロセス全体が落ちる。C 側の OS リソース (V4L2 バッファ、AVF ピクセルバッファ、PipeWire ストリームバッファ) が正常に返却されない
- `src/video.h:37` で「コールバックはこの FFI 境界を跨いでアンワインド（パニック）してはならない」と C ABI 利用者向けに契約しているが、Rust 側実装が `extern "C" fn frame_callback` 内でこの契約を保証していない。Rust 側実装の責務として `catch_unwind` で守る必要がある
- ユーザ側のごく軽微なバグ (`unwrap()` 失敗、整形失敗) が直ちにプロセス abort と C 側リソース欠損を引き起こす
- `/review-code` の致命的指摘として確認されたバグ

## 現状

### `src/capture_ffi.rs:373` の `frame_callback`

`extern "C" fn frame_callback` (`src/capture_ffi.rs:248-374`) の終端 L373 で `(context.callback)(frame);` を panic 保護なしで呼んでいる。

```rust
extern "C" fn frame_callback(/* ... */) {
    /* バッファ妥当性チェック・スライス取得 */
    let frame = match pf { /* ... */ };
    (context.callback)(frame);   // ← panic 保護なし
}
```

panic で unwind すると以下の C 側後処理がスキップされる:

- **V4L2**: `video_v4l2.c:648` の `VIDIOC_QBUF` (バッファをカーネルに返却する `ioctl`) が呼ばれず、V4L2 のバッファプールが枯渇してキャプチャが止まる。mmap 領域自体はプロセス abort 時に OS が回収するが、稼働中のセッションはバッファ欠損で機能不全になる
- **AVFoundation**: `video_avf.m` の `CVPixelBufferUnlockBaseAddress` が呼ばれず、ロックが残ったままになる
- **PipeWire**: `video_pipewire.c:511` の `pw_stream_queue_buffer` がスキップされ、PipeWire のストリームバッファが返却されず枯渇する (`pw_thread_loop` のロック自体は `on_process` 呼び出し時点では取得されていないため、ロック残留はない)

### `src/capture_mf.rs:580` の `process_sample`

`unsafe fn process_sample` (`src/capture_mf.rs:455-582`) の L580 で `callback(frame);` を panic 保護なしで呼んでいる。Rust の `thread::spawn` 経由のため abort にはならず、`BufGuard` (`IMFMediaBuffer::Unlock` の RAII) も巻き戻されるが、`capture_thread_func` が panic で終了する結果:

- `JoinHandle::join()` が `Err` を返す
- 0022 で追加する `MfCaptureImpl::stop` の `eprintln!` ログで可視化はされるが、callback は永久消失して `VideoCapture` の再 `start` が黙って no-op になる

panic を抑止することで、0022 のログ可視化に依存せずに「再 `start` できる契約」を維持できる。

## 設計方針

### 1. `frame_callback` (`src/capture_ffi.rs:373`) で `catch_unwind` 保護

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

static USER_CALLBACK_PANIC_LOGGED_FFI: AtomicBool = AtomicBool::new(false);

extern "C" fn frame_callback(/* ... */) {
    /* バッファ妥当性チェック・スライス取得・frame 構築は現状のまま */
    if catch_unwind(AssertUnwindSafe(|| (context.callback)(frame))).is_err()
        && !USER_CALLBACK_PANIC_LOGGED_FFI.swap(true, Ordering::Relaxed)
    {
        eprintln!(
            "user frame callback panicked at FFI boundary; further panics in this process are silenced"
        );
    }
}
```

- `extern "C"` 境界を越える unwind を抑止する (Rust 2024 edition で abort になるため)
- panic payload (`Box<dyn Any>`) は捨てる。利用者に届ける手段がない (FFI 境界・スレッド境界を越えるため)
- `AssertUnwindSafe` を使う根拠:
  - `Box<dyn Fn(VideoFrame<'_>) + Send + 'static>` 自体は `UnwindSafe` を実装しない (`dyn Fn` には `UnwindSafe` 補助マーカーが付かない)
  - ユーザコールバックがキャプチャする内部状態が `UnwindSafe` を満たすかも実装依存で確認できない
  - `AssertUnwindSafe` で両方の制約を意図的に外す。panic 後も同じ `Box<dyn Fn>` を呼び続けるため、論理的に破壊された状態 (broken invariant) のままユーザコールバックが再呼び出しされる懸念は残るが、**FFI 境界を越える abort を抑止することの優先度が高いため、このトレードオフを受け入れる**
  - `VideoFrame.pixel_buffer: Option<PixelBuffer>` の `Drop` で AVF の `CFRelease` が呼ばれるが、`CFRelease` は panic しない C API なので unwind 中でも安全に走る
- ログ抑制に `AtomicBool::swap` を使う理由:
  - フレームごとに `eprintln!` を呼ぶと、コールバックが毎フレーム panic するケースで stderr が氾濫しキャプチャ自体の安定性を損なう
  - `std::sync::Once::call_once` の利用は避ける。`call_once` のクロージャが panic すると `Once` が poison 状態になり、以後の `call_once` 呼び出しが panic する。`eprintln!` は stderr 書き込み失敗時に内部で panic する (`failed printing to stderr` で `panic!`) ため、`Once` を使うと panic 連鎖を招きうる
  - `AtomicBool::swap(true, Ordering::Relaxed)` は poison 概念がなく、必ず前回値を返す。`eprintln!` が万一 panic しても abort には至らず (上位の `catch_unwind` で吸収済み)、論理状態も壊れない
  - 1 回に絞ると後続の panic を観測できなくなる代償はある。将来カウンタ + 周期出力 (`AtomicU64` + N 回ごと等) や `tracing` 移行を検討する余地があるが、本 issue では扱わない

### 2. `process_sample` (`src/capture_mf.rs:580`) で同じ保護

```rust
static USER_CALLBACK_PANIC_LOGGED_MF: AtomicBool = AtomicBool::new(false);

// ... process_sample 内、callback 呼び出し位置 ...
if catch_unwind(AssertUnwindSafe(|| callback(frame))).is_err()
    && !USER_CALLBACK_PANIC_LOGGED_MF.swap(true, Ordering::Relaxed)
{
    eprintln!(
        "user frame callback panicked in MF capture thread; further panics in this process are silenced"
    );
}
```

- `frame` は move キャプチャ、`callback: &VideoFrameCallback` は参照キャプチャ
- panic 吸収後は `capture_thread_func` のループが継続するため、ユーザコールバック起因で `JoinHandle::join()` が `Err` を返すことはなくなる
- 0022 で追加する `MfCaptureImpl::stop` 内の `eprintln!` ログは、本 issue 適用後はユーザコールバック起因の panic 経路では到達しなくなる。ただし `process_sample` 内の `unsafe` COM 呼び出し (`ConvertToContiguousBuffer`、`Lock` 等) や `source_reader.ReadSample` 由来の panic 経路は残り、その診断手段として 0022 のログは引き続き有効
- ログ用 static は `capture_ffi.rs` と独立した別の `AtomicBool` を使う。経路を区別したログを得るため。同じ static にすると、どちらの経路で panic したか区別できなくなる

### インポート追加

`src/capture_ffi.rs` および `src/capture_mf.rs` に以下を追加する:

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
```

`Cargo.toml` への依存追加は不要 (`std::panic`、`std::sync::atomic` は標準ライブラリ)。

### スコープ外

- `src/video.h:37` の C ABI 契約コメントは現状のまま (C 側 ABI 利用者向け契約であり、本 issue は Rust 側がその契約を実装側で保証する修正)
- C 側ヘルパ関数 (`video_v4l2.c`、`video_pipewire.c`、`video_avf.m`) の改修は不要 (Rust 側で panic を吸収すれば、C 側は通常リターンを受け取って後処理を実行する)
- `AtomicBool` を `AtomicU64` カウンタ + 周期出力に置き換える改善や `tracing` への移行は本 issue では扱わない。必要なら別 issue (refactor / change カテゴリ) を起票する

## 影響範囲

- `src/capture_ffi.rs`: L373 の `(context.callback)(frame);` を `catch_unwind` で囲む。`AtomicBool` ログ用 static 1 つを追加
- `src/capture_mf.rs`: L580 の `callback(frame);` を `catch_unwind` で囲む。`AtomicBool` ログ用 static 1 つを追加
- `src/types.rs`, `src/device_mf.rs`, `src/capture.rs`, `src/error.rs`, C 側ファイル: 変更なし
- 公開 API: 変更なし (`Fn(VideoFrame<'_>)` バウンドは不変。`UnwindSafe` 制約は追加しない)
- 0021 (`MfCaptureImpl::new`) と 0022 (`MfCaptureImpl::stop` / `start`) と同じ `src/capture_mf.rs` を触るが、変更箇所は別関数のためマージ順序の制約はない。コンフリクトが起きた場合は手動でマージする
- 0022 との関係: 本 issue 適用後、0022 が追加する `MfCaptureImpl::stop` 内の panic ログ (`handle.join() == Err` 経路) は **ユーザコールバック起因の panic 経路** では到達しなくなる。ただし `process_sample` 内の `unsafe` COM 呼び出し (`ConvertToContiguousBuffer`、`Lock` 等) や `source_reader.ReadSample` 由来の panic 経路は残るため、0022 のログは引き続き診断手段として有効

## 完了条件

- `src/capture_ffi.rs::frame_callback` 内のユーザコールバック呼び出し (`(context.callback)(frame);`) を `catch_unwind(AssertUnwindSafe(...))` で囲み、`Err` 時に専用 `AtomicBool` static (`USER_CALLBACK_PANIC_LOGGED_FFI`) を `swap(true, Ordering::Relaxed)` で確認し、前回値が `false` の場合のみ `eprintln!` ログを出力すること
- `src/capture_mf.rs::process_sample` 内のユーザコールバック呼び出し (`callback(frame);`) を同様に囲み、`capture_ffi.rs` とは別の `AtomicBool` static (`USER_CALLBACK_PANIC_LOGGED_MF`) を使って同じパターンでログを出力すること。ログ文言で `at FFI boundary` / `in MF capture thread` を区別する
- ログ抑制に `std::sync::Once` を使わないこと (`call_once` の poison 挙動と `eprintln!` の stderr 書き込み失敗時 panic の組み合わせで panic 連鎖を起こしうるため)
- ログメッセージは英語 (AGENTS.md 規約)、コメントは日本語
- `Cargo.toml` への依存追加を行わない (`std::panic`、`std::sync::atomic` は標準ライブラリ)
- 既存の正常系挙動 (コールバックが panic しない場合、`frame_callback` および `process_sample` の戻り値・副作用とも不変) を維持すること
- 各バックエンドの対象 OS 上で `cargo test --workspace` および `--ignored` テストが通る (`default-v4l2` は Linux、`default-avf` は macOS、`default-mf` は Windows でそれぞれ検証する。全 feature を 1 OS で同時有効化することは現状不可能)
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- 新規テストは本 issue では追加しない (`frame_callback` を Rust テストから直接呼び出すには `CaptureContext` を実機セッションつきで構築する必要があり、本リポジトリの test_capture.rs 構造に乗らない。`process_sample` は `unsafe fn` で `IMFSample` を要求するためテストから直接呼べない。closed/0009, closed/0018, 0021, 0022 と同様にコードレビューで分岐網羅と panic 吸収の正しさを担保する)
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] ユーザフレームコールバックの panic が FFI 境界を跨いでプロセスを abort させる問題を catch_unwind で修正する`)。担当者行 (`- @<github-id>`) を含める

## 解決方法

{完了時に記入}
