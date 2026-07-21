# Windows MfCaptureImpl::new のエラーパスで CoUninitialize が MFShutdown より先に呼ばれる

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-mf-com-shutdown-order
- Polished: 2026-07-21

## 目的

`MfCaptureImpl::new()`（`src/capture_mf.rs:138-197`）の構築クロージャが失敗したとき、`move` クロージャが捕捉した `CoInitGuard` の drop により `CoUninitialize` が先に走り、その後の `MFShutdown()` が COM 未初期化状態で呼ばれる。MSDN の COM/MF 初期化契約に違反しており、同一プロセス内の後続 COM/MF 操作がクラッシュ・リーク・未定義動作を引き起こしうる。この解放順序を修正する。

## 優先度根拠

- High。COM/MF の初期化契約違反であり、クラッシュ・UB の経路
- `MfCaptureImpl::new()` のエラーパス（デバイス未接続、フォーマット非対応等）で必ず発生する
- `/review-code` の致命的指摘として確認

## 現状

`src/capture_mf.rs:145-196`:

```rust
let com_guard = CoInitGuard::new()?;                    // CoInitializeEx
MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(...)?; // MF 初期化

let result: Result<Self> = (move || -> Result<Self> {
    // com_guard をクロージャが move で捕捉
    // ...
    Ok(Self { ..., _com_guard: com_guard })  // Ok 時のみ move out
})();

if result.is_err() {
    let _ = MFShutdown();  // ← Err 時: クロージャ drop で CoUninitialize 済み
}
```

`move` クロージャが `com_guard` を捕捉し、`Ok` パスでのみ `Self` に move out する。`Err` を返す `?` 経路（`activate_device` 失敗、`create_source_reader` 失敗等）では `com_guard` はクロージャ内に残存し、`let result = ...;` 文の終わりでクロージャ一時オブジェクトが drop され `CoUninitialize` が走る。その後の `MFShutdown()` は COM が既に未初期化の状態で呼ばれる。

## 設計方針

`com_guard` をクロージャの外で保持し、エラーパスで `MFShutdown()` の **後に** drop されるようにする。

採用案: クロージャの戻り値を `Result<SessionData>` にし、`Self` の構築をクロージャ外で行う。これにより `com_guard` はクロージャに捕捉されず、Err 時は `MFShutdown()` → `com_guard` drop の順序が自然に保証される。`config` / `callback` もクロージャ外で `Self` に move できる。

```rust
let com_guard = CoInitGuard::new()?;
MFStartup(...)?;

// クロージャは SessionData の構築のみ。com_guard / callback は捕捉しない（config は参照で借用）
let result: Result<SessionData> = (|| -> Result<SessionData> {
    let media_source = activate_device(config.device_id.as_deref())?;
    let source_reader = create_source_reader(...)?;
    let (pixel_format, width, height) = get_configured_format(&source_reader)?;
    // ...
    Ok(SessionData { ... })
})();

match result {
    Ok(session) => Ok(Self {
        session: Some(session),
        running: Arc::new(AtomicBool::new(false)),
        callback: Some(Box::new(callback)),
        capture_thread: None,
        config,
        _com_guard: com_guard,
    }),
    Err(e) => {
        let _ = MFShutdown();
        Err(e)
    }
}
```

### 採用しない案

- クロージャを `move` にせず構築ロジックを通常のブロックに展開する: 非 move クロージャでは `config` / `callback` を `Self` に move できないため、そのままではコンパイルが通らない
- `com_guard` を `Option<CoInitGuard>` にし `take()` で取り出す: `move` クロージャと組み合わせると Option 全体がクロージャに捕捉され、Err 時にクロージャ drop で `Some(CoInitGuard)` が drop されるため機能しない
- `MFShutdown()` を `CoInitGuard::drop` の前に呼ぶことを型で強制する: 複雑すぎる。順序をコードで保証すれば十分

### 後方互換への影響

なし。`MfCaptureImpl::new()` の外部 API（引数・戻り値）は不変。内部の解放順序のみの修正。

### 他 issue との関係

0037（`capture_mf.rs` の状態管理強化）と同じファイルを触るが、変更箇所が異なる（0030 は `new()`、0037 は `capture_thread_func` と `Drop`）。マージ順序の制約はない。

## 完了条件

- `MfCaptureImpl::new()` のエラーパスで `MFShutdown()` が `CoUninitialize` より先に呼ばれる
- `MfCaptureImpl::new()` の成功パスで `CoInitGuard` が `Self._com_guard` に正しく格納される
- `cargo build --workspace`（Windows、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
