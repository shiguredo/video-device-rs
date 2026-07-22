# Windows MfCaptureImpl::new で MFStartup と MFShutdown が対にならず参照カウントが残る

- Priority: High
- Created: 2026-06-15
- Completed: 2026-06-16
- Model: Opus 4.7
- Branch: feature/fix-mf-startup-shutdown-balance
- Polished: 2026-06-15

## 目的

`MfCaptureImpl::new` (`src/capture_mf.rs`) の構築途中エラー経路で `MFShutdown` が呼ばれず、Media Foundation の `MFStartup` 参照カウントが残ったままになるリソースリークを修正する。

## 優先度根拠

- High。Media Foundation の参照カウントが残るとプロセス内で MF 状態異常を引き起こしうる
- `VideoCapture::new` の Windows バックエンド構築失敗を繰り返すと参照カウントが積み上がり、最悪 MF サブシステムが再初期化できなくなる
- MSDN の `MFStartup` / `MFShutdown` 契約 (成功した `MFStartup` には必ず対の `MFShutdown` を呼ぶ) に直接違反する
- `/review-code` の致命的指摘として確認されたバグ

## 現状

`MfCaptureImpl::new` (`src/capture_mf.rs` の `impl MfCaptureImpl { pub fn new<F>(...) -> Result<Self> ... }`) は以下の構造になっている。

```rust
unsafe {
    let com_guard = CoInitGuard::new()?;
    MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|_| Error::SessionCreateFailed)?;
    let result = {
        let media_source = activate_device(config.device_id.as_deref())?;
        let source_reader = create_source_reader(/* ... */)?;
        let (pixel_format, width, height) = get_configured_format(&source_reader)?;
        if matches!(pixel_format, PixelFormat::Unknown(_)) {
            return Err(Error::UnsupportedPixelFormat(pixel_format));
        }
        // ...
        Ok(Self { /* ... */ })
    };
    // 構築途中で失敗した場合は MFStartup とつりあわせるために MFShutdown を呼ぶ
    if result.is_err() {
        let _ = MFShutdown();
    }
    result
}
```

コード上は対策のつもりのコメントと `if result.is_err() { let _ = MFShutdown(); }` 分岐が **すでに書かれている** が、これは **死コードであり実際には到達しない**。Rust の `?` と明示的な `return` はブロック式ではなく囲んでいる関数 `MfCaptureImpl::new` から return するため、`activate_device` / `create_source_reader` / `get_configured_format` の失敗または `Unknown` ピクセルフォーマット拒否時には外側の `if result.is_err()` 分岐に到達せず `MFShutdown` が呼ばれない。結果、`MFStartup` の参照カウントが 1 残る。

## 設計方針

`?` と `return` のスコープを内側ブロックに閉じ込めるため、`let result = { ... }` のブロック式を **`move` 即時実行クロージャ** に置き換える。

```rust
unsafe {
    let com_guard = CoInitGuard::new()?;
    MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|_| Error::SessionCreateFailed)?;

    // クロージャに切り出すことで `?` および明示的 `return Err(...)` の return 先を
    // 関数 `new` ではなくクロージャ自身に限定する。
    // Rust 2024 edition では `unsafe { ... }` 内のクロージャ本体は unsafe コンテキストを
    // 引き継がないため、クロージャ内で再度 `unsafe { ... }` を書く。
    // `com_guard` / `callback` / `config` をクロージャ内に move する必要があるため `move` を付ける。
    let result: Result<Self> = (move || -> Result<Self> {
        unsafe {
            let media_source = activate_device(config.device_id.as_deref())?;
            let source_reader = create_source_reader(
                &media_source,
                config.width,
                config.height,
                config.fps,
                config.pixel_format,
            )?;
            let (pixel_format, width, height) = get_configured_format(&source_reader)?;
            if matches!(pixel_format, PixelFormat::Unknown(_)) {
                return Err(Error::UnsupportedPixelFormat(pixel_format));
            }
            let running = Arc::new(AtomicBool::new(false));
            let session = SessionData {
                source_reader,
                media_source,
                pixel_format,
                width,
                height,
            };
            Ok(Self {
                session: Some(session),
                running,
                callback: Some(Box::new(callback)),
                capture_thread: None,
                config,
                _com_guard: com_guard,
            })
        }
    })();

    if result.is_err() {
        let _ = MFShutdown();
    }
    result
}
```

これにより `?` も `return Err(...)` もクロージャから return され、`if result.is_err() { let _ = MFShutdown(); }` のチェックに必ず到達する。

### 参照カウント・所有権の対応

- 成功時: クロージャは `Ok(Self { ..., _com_guard: com_guard })` を返し、`com_guard` は `Self` のフィールドに移される。`if result.is_err()` は偽となり `MFShutdown` は呼ばれない。`MFStartup` 1 回 + 「`Self` の所有期間中の使用」+ 「`Drop for MfCaptureImpl` 内の `MFShutdown` 1 回」で対称が成立する。`Drop for MfCaptureImpl` は変更しない
- 失敗時 (クロージャ内): `?` または `return Err(...)` でクロージャから return し、`com_guard` はクロージャ末尾でドロップされて `CoUninitialize` が走る。続けて `if result.is_err()` が真となり `MFShutdown` が呼ばれて `MFStartup` 1 回を相殺する。`Self` は構築されないため `Drop` は走らない
- `MFStartup` 自体の失敗 (クロージャに到達しない): `?` で `MfCaptureImpl::new` 関数から return する。`com_guard` は関数末尾でドロップされて `CoUninitialize` が走る。`MFShutdown` は呼ばない (MSDN の規約: 失敗した `MFStartup` には対の `MFShutdown` を呼んではいけない)

### 採用しない案

`MFStartup` / `MFShutdown` を RAII ガード型 (`MfStartupGuard` 等) に切り出す案は、本 bug 修正のスコープでは採用しない。

- ローカル変数として束縛する RAII 案は `mem::forget` 等の追加機構が必要で、即時実行クロージャに比べて変更量が増える
- 構造体フィールドに保持する RAII 案は `MfCaptureImpl::new` の失敗パスでは `Self` が構築されず `Drop` が呼ばれないため、本バグの修正にはならない
- `issues/closed/0019-api-trait-videodevice-videocapture.md` で同種のガード型 (`MfShutdownGuard`) を削除し、`new()` 内の inline 方式へ統一する判断が既に下されている。本 issue はその方針と整合する

RAII ガード型による構造改善を別途進めたい場合は refactor カテゴリの新規 issue として起票すること (本 issue では起票しない)。

## 影響範囲

- `src/capture_mf.rs`: `MfCaptureImpl::new` 内のブロック式を `move` 即時実行クロージャに置き換える 1 箇所のみ
- `src/device_mf.rs`: 変更なし (`enumerate_devices_internal` は `?` を直接戻り値に伝播させていないため既に `MFShutdown` 漏れを起こさない構造)
- `src/types.rs`, `src/error.rs`, `Drop for MfCaptureImpl`: 変更なし
- 公開 API: 変更なし (`VideoCapture::new` のシグネチャは不変)
- 0022 (`MfCaptureImpl::start`/`stop` の修正) および 0023 (`process_sample` の panic 保護) と同じ `src/capture_mf.rs` を触るが、変更箇所は別関数のためマージ順序の制約はない。コンフリクトが起きた場合は手動でマージする

## 完了条件

- `MfCaptureImpl::new` の構築途中失敗経路 (`activate_device` の失敗、`create_source_reader` の失敗、`get_configured_format` の失敗、`Unknown` ピクセルフォーマット拒否) で `MFShutdown` が必ず呼ばれること。コードレビューで `?` および `return Err(...)` が `move` クロージャから return することを確認する
- 正常系挙動 (構築成功時は `MFShutdown` を呼ばず、`Drop for MfCaptureImpl` で `MFShutdown` を呼ぶ) を維持すること
- `MFStartup` 自体が失敗した場合は `MfCaptureImpl::new` から `?` で return し、`MFShutdown` を呼ばないこと
- `cargo test --workspace` および Windows 環境での `cargo test --workspace -- --ignored` が通る (新規テスト追加は本 issue では行わない。closed/0010, closed/0018 と同様に、Drop 経路の網羅検証はコードレビューで担保する)
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] Windows で VideoCapture 構築途中失敗時に Media Foundation の参照カウントが残るのを修正する`)。担当者行 (`- @<github-id>`) を含める

## 解決方法

`MfCaptureImpl::new` 内の `let result = { ... }` ブロック式を `move` 即時実行クロージャに置き換えた。これにより構築途中の `?` および `return Err(...)` の return 先がクロージャに限定され、`activate_device` / `create_source_reader` / `get_configured_format` の失敗時や `Unknown` ピクセルフォーマット拒否時でも `if result.is_err()` 分岐に到達し `MFShutdown` が必ず呼ばれるようになった。`MFStartup` 自体の失敗時は従来通り `MfCaptureImpl::new` から直接 return し `MFShutdown` を呼ばない。正常系 (`MFStartup` 成功 → `Self` 構築成功) では `Drop for MfCaptureImpl` 内の `MFShutdown` で対消滅する。
