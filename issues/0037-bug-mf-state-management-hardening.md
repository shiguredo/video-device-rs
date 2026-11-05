# Windows MF 状態管理のハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-mf-state-management-hardening
- Polished: 2026-11-04

## 目的

Windows Media Foundation バックエンド（`src/capture_mf.rs`）の状態管理に 2 つの不備がある。(1) キャプチャスレッドの COM 初期化失敗時に `running` フラグが true のまま残留し、再 `start()` が `Ok(())` を返すがフレームが来ない、(2) `Drop` で `source_reader` の解放前に `media_source.Shutdown()` を呼んでいる。これらを修正する。

## 優先度根拠

- Medium。(1) は COM 初期化失敗（稀）後の状態不整合、(2) は MF の推奨される解放順序に反している
- (1) はユーザーから「start 済みなのにフレームが来ない」状態が観測される
- (2) は source reader の解放時に media source が再度 Shutdown され、不要な二重 Shutdown やエラーコードの原因になり得る
- `/review-code` の重要指摘として確認

## 現状

### (1) COM 初期化失敗時に `running` が true のまま（`src/capture_mf.rs` の `MfCaptureImpl::start` と `capture_thread_func`）

`MfCaptureImpl::start` で `self.running.store(true, Ordering::Release)` を設定してからスレッドを起動する。`capture_thread_func` で `CoInitGuard::new()` が失敗すると、スレッドは即座に `callback` を返して終了するが、`running` は true のまま。

このとき `stop()` を呼ばずに `start()` を再度呼ぶと、`MfCaptureImpl::start` の `if self.running.load(Ordering::Acquire) { return Ok(()); }` が true を返し、キャプチャスレッドは死んでいるのに「start 済み」となる。

### (2) `Drop` で source_reader の解放前に `media_source.Shutdown()`（`src/capture_mf.rs` の `impl Drop for MfCaptureImpl`）

```rust
impl Drop for MfCaptureImpl {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            unsafe {
                let _ = session.media_source.Shutdown();  // ← 先に Shutdown
            }
        }
        // if let ブロック終了時に session のフィールドが drop され、source_reader が COM release される ← 後
        unsafe {
            let _ = MFShutdown();
        }
    }
}
```

Media Foundation では、`MFCreateSourceReaderFromMediaSource` で作成した source reader は、`MF_SOURCE_READER_DISCONNECT_MEDIASOURCE_ON_SHUTDOWN` が FALSE（既定値）の場合、解放時に `IMFMediaSource::Shutdown` を呼ぶ。そのため、source reader の解放より先に media source を shutdown すると、source reader の解放処理が shutdown 済みの media source に対して再度 `Shutdown` を呼ぶことになる。source reader を先に解放する。

## 設計方針

### (1) の修正

`capture_thread_func` が COM 初期化失敗で早期終了する場合、`running` を false に設定し、診断ログを出力してから戻る。ログ出力が失敗して panic しても `running` が true のまま残らないよう、フラグを先に更新する。

```rust
fn capture_thread_func(...) -> VideoFrameCallback {
    let _com_guard = match CoInitGuard::new() {
        Ok(g) => g,
        Err(_) => {
            running.store(false, Ordering::Release);
            eprintln!("capture thread failed to initialize COM; capture may not be restartable");
            return callback;
        }
    };
    // ...
}
```

修正後の挙動: `capture_thread_func` が COM 初期化失敗で `running` を false にした場合、`MfCaptureImpl::stop` は running が false のため早期 return し、`JoinHandle` 内の callback は回収されない（`MfCaptureImpl` の drop 時に `JoinHandle` とともに廃棄される）。そのため、`stop()` を挟まずに再 `start()` を呼ぶと `Err(CaptureFaulted)` になる。一方、`stop()` が先に `running=false` を設定して `join()` した場合は callback が回収されるため、再 `start()` は可能である（COM 初期化に再度失敗する可能性はある）。このように、COM 初期化失敗後の再 `start()` の可否は `stop()` との競合タイミングに依存する。`running` は `AtomicBool` なので、どちらが先に `running=false` を設定してもデータ競合や UB にはならない。

ログ出力: panic 時は `MfCaptureImpl::stop` の `handle.join()` が `Err` を返す分岐でログを出すのに対し、COM 失敗時は `capture_thread_func` 側でログを出す。

後方互換: 修正前は COM 初期化失敗後に `stop()` を挟まず再 `start()` すると `Ok(())` を返していた（フレームは来ないがエラーではない）。修正後は、`capture_thread_func` が `running` を false にした後で `stop()` を挟まず再 `start()` すると `Err(CaptureFaulted)` になる。`stop()` が先に `running=false` を設定した場合は従来どおり再 `start()` できる。バグ修正として許容されるが、CHANGES.md の `[FIX]` エントリで挙動変化に触れる。

### (2) の修正

`Drop` で `source_reader` を明示的に drop してから `media_source.Shutdown()` を呼ぶ。`SessionData` は `Drop` を実装していないため、`source_reader` の部分 move が可能。

```rust
if let Some(session) = self.session.take() {
    drop(session.source_reader);  // 先に source reader を解放
    unsafe {
        let _ = session.media_source.Shutdown();
    }
}
```

### 他 issue との関係

0030（`capture_mf.rs` の `new()` エラーパスの COM/MF 解放順序）と同じファイルを触るが、変更箇所が異なる（0030 は `new()`、0037 は `capture_thread_func` と `Drop`）。マージ順序の制約はない。

0055（MF のキャプチャスレッドが panic 後に再 start で成功を返し、ReadSample のエラーを無限リトライする）も `capture_thread_func` を変更する。0055 はキャプチャスレッド終了時に `running` を false にする RAII ガードを導入する方針（または `start()` で `capture_thread` の終了を確認する方針）のため、RAII ガード方式が採用された場合は (1) は解消済みとなり、(2) のみ実施すればよい。0037 を先に実装する場合、0055 の RAII ガード導入時に (1) の手動 `running.store(false, ...)` が重複する可能性がある。どちらを先にマージしてもよいが、後から実装する側で重複を解消すること。

## 完了条件

- (1) COM 初期化失敗時に `running` を false に設定し、診断ログを stderr に出力する
- (2) `Drop` で `source_reader` を `media_source.Shutdown()` より先に解放する
- `Error::CaptureFaulted` の `Display` と、`src/capture.rs` / `src/capture_mf.rs` / `src/lib.rs` / README の Windows の記述を、COM 初期化失敗でも `stop()` を挟まない再 `start()` が `CaptureFaulted` になり得る旨に更新する
- `cargo build --workspace`（Windows、default features）が通る
- `cargo fmt --all --check` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する（担当者行 `- @<github-id>` を含める）

## 解決方法

{完了時に記入}
