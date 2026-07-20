# Windows MfCaptureImpl::new のエラーパスで CoUninitialize が MFShutdown より先に呼ばれる

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-mf-com-shutdown-order
- Polished: {YYYY-MM-DD}

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

```rust
let com_guard = CoInitGuard::new()?;
MFStartup(...)?;

let result: Result<Self> = (|| -> Result<Self> {
    // com_guard を move しない。借用のみ
    // ...
    Ok(Self { ..., _com_guard: /* ここだけ move が必要 */ })
})();
```

`Ok` パスでは `com_guard` を `Self` に move する必要があるため、クロージャの構造を見直す。具体的には:

1. クロージャを `move` にせず、構築ロジックを通常のブロックに展開する
2. `Ok` 時に `com_guard` を `Self` に move し、`Err` 時は `MFShutdown()` の後に `com_guard` が drop される順序を保証する
3. または `com_guard` を `Option<CoInitGuard>` にし、`Ok` 時に `take()` で取り出す

### 採用しない案

- `MFShutdown()` を `CoInitGuard::drop` の前に呼ぶことを型で強制する: 複雑すぎる。順序をコードで保証すれば十分

## 完了条件

- `MfCaptureImpl::new()` のエラーパスで `MFShutdown()` が `CoUninitialize` より先に呼ばれる
- `MfCaptureImpl::new()` の成功パスで `CoInitGuard` が `Self._com_guard` に正しく格納される
- `cargo build --workspace`（Windows、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
