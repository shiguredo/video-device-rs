# Windows MF 状態管理のハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-mf-state-management-hardening
- Polished: {YYYY-MM-DD}

## 目的

Windows Media Foundation バックエンド（`src/capture_mf.rs`）の状態管理に 2 つの不備がある。(1) キャプチャスレッドの COM 初期化失敗時に `running` フラグが true のまま残留し、再 `start()` が `Ok(())` を返すがフレームが来ない、(2) `Drop` で `source_reader` の解放前に `media_source.Shutdown()` を呼んでいる。これらを修正する。

## 優先度根拠

- Medium。(1) は COM 初期化失敗（稀）後の状態不整合、(2) は MF の解放順序契約違反
- (1) はユーザーから「start 済みなのにフレームが来ない」状態が観測される
- (2) は shutdown 済みの media source に対して source reader のデストラクタが内部処理を試み、未定義の振る舞いやエラーログの原因になる
- `/review-code` の重要指摘として確認

## 現状

### (1) COM 初期化失敗時に running が true のまま（src/capture_mf.rs:77, 412-414）

`start()`（:77）で `self.running.store(true, Ordering::Release)` を設定してからスレッドを起動する。`capture_thread_func`（:412-414）で `CoInitGuard::new()` が失敗すると、スレッドは即座に `callback` を返して終了するが、`running` は true のまま。

このとき `stop()` を呼ばずに `start()` を再度呼ぶと、:69 の `if self.running.load(Ordering::Acquire) { return Ok(()); }` が true を返し、キャプチャスレッドは死んでいるのに「start 済み」となる。

### (2) Drop で source_reader の解放前に media_source.Shutdown()（src/capture_mf.rs:200-213）

```rust
impl Drop for MfCaptureImpl {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            unsafe {
                let _ = session.media_source.Shutdown();  // ← 先に Shutdown
            }
        }
        // session の drop で source_reader が COM release される ← 後
        unsafe {
            let _ = MFShutdown();
        }
    }
}
```

MF の文書では source reader の flush/release を media source の shutdown より先に行うべき。

## 設計方針

### (1) の修正

`capture_thread_func` が COM 初期化失敗で早期終了する場合、`running` を false に設定してから戻る。

```rust
fn capture_thread_func(...) -> VideoFrameCallback {
    let _com_guard = match CoInitGuard::new() {
        Ok(g) => g,
        Err(_) => {
            running.store(false, Ordering::Release);
            return callback;
        }
    };
    // ...
}
```

### (2) の修正

`Drop` で `source_reader` を明示的に drop してから `media_source.Shutdown()` を呼ぶ。

```rust
if let Some(session) = self.session.take() {
    drop(session.source_reader);  // 先に source reader を解放
    unsafe {
        let _ = session.media_source.Shutdown();
    }
}
```

## 完了条件

- (1) COM 初期化失敗時に `running` を false に設定する
- (2) `Drop` で `source_reader` を `media_source.Shutdown()` より先に解放する
- `cargo build --workspace`（Windows、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
