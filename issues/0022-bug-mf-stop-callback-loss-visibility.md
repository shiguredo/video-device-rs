# Windows MfCaptureImpl::stop でキャプチャスレッド panic 時に callback が黙って消失する

- Priority: High
- Created: 2026-06-15
- Completed: 2026-07-15
- Model: Opus 4.7
- Branch: feature/fix-mf-stop-callback-loss-visibility
- Polished: 2026-06-15

## 目的

`MfCaptureImpl::stop` (`src/capture_mf.rs`) で `handle.join()` が `Err` を返した場合 (キャプチャスレッドが panic した場合)、`self.callback` が `None` のまま `running = false` で終了し、その後の再 `start` が `self.callback.is_none()` 判定で黙って `Ok(())` を返してしまう。利用者からは「再 `start` が成功したのにキャプチャが始まらない」状態が観測されるが、その原因 (内部状態の破壊) を知る手段がない。本 issue ではこの内部状態破壊をエラーログで可視化し、同時に `start` の `callback.take()` 順序を防御的に整える。

## 優先度根拠

- High。`VideoCapture` の公開ドキュメント (`src/capture.rs` の `VideoCapture::start` doc コメント) が「`start` は冪等で `stop` 後の再 `start` を許容する」という契約を約束している。本バグはこの契約を一部のシナリオで破る
- バグの兆候 (再 `start` が `Ok(())` を返すのにフレームが流れない) が観察不能であり、利用者は回避策を取れない
- `/review-code` の致命的指摘として確認されたバグ
- FFI コールバックの panic 保護 (別 issue) が完了すればキャプチャスレッドの panic 経路は実質的に塞がれるが、それまでの間および将来の panic 経路追加に対する診断手段が必要

## 現状

`MfCaptureImpl::stop` (`src/capture_mf.rs` の `pub fn stop(&mut self) { ... }`) は以下の構造で、`handle.join()` の `Err` を握り潰している。

```rust
pub fn stop(&mut self) {
    if self.running.load(Ordering::Acquire) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.capture_thread.take()
            && let Ok(callback) = handle.join()
        {
            self.callback = Some(callback);
        }
    }
}
```

`handle.join()` が `Err` を返す経路 (キャプチャスレッド内のユーザコールバックが panic した場合) では、`self.callback` が `None` のまま残り、`running` も `false` に戻る。その後利用者が `start()` を呼ぶと:

```rust
pub fn start(&mut self) -> Result<()> {
    let session = self.session.as_ref().ok_or(Error::SessionStartFailed)?;
    let callback = self.callback.take();
    if callback.is_none() || self.running.load(Ordering::Acquire) {
        return Ok(());
    }
    // ...
}
```

`self.callback` は `None` なので `callback.is_none()` が真となり `Ok(())` を返す。利用者は API レベルで成功を見るのにキャプチャは再開しない。

加えて `start()` の `let callback = self.callback.take();` を判定より前に置いている点も防御的修正の対象とする (詳細は設計方針 2)。

## 設計方針

### 1. `stop` で `handle.join() == Err` を検知してエラーログを出す (本 issue の主修正)

`handle.join()` が `Err` を返す経路で内部状態が破壊されたことを `eprintln!` で出力し、ユーザがログから原因を追えるようにする。callback そのものは復元できない (panic でスレッドごと巻き戻っているため)。

```rust
pub fn stop(&mut self) {
    if !self.running.load(Ordering::Acquire) {
        return;
    }
    self.running.store(false, Ordering::Release);
    if let Some(handle) = self.capture_thread.take() {
        match handle.join() {
            Ok(callback) => {
                self.callback = Some(callback);
            }
            Err(_) => {
                // キャプチャスレッドが panic した経路。callback は永久消失し、
                // その後の再 start は黙って Ok(()) を返すため、診断手段として
                // エラーログを残す。将来 capture_thread_func のユーザ callback を
                // catch_unwind で保護する変更が入れば、この経路には到達しなくなる。
                eprintln!(
                    "capture thread panicked; callback is permanently lost and capture cannot be restarted"
                );
            }
        }
    }
}
```

- ログメッセージは AGENTS.md 規約「ログメッセージは全て英語」に従い英語。コメントは日本語
- `Err(_)` で panic payload (`Box<dyn Any + Send>`) は捨てる。`downcast` でメッセージを取り出すアイデアもあるが、本 issue では追加せず「panic 発生の事実のみログに残す」最小実装とする
- `log` クレート等のランタイム依存追加は行わない。`Cargo.toml` の `[dependencies]` は空のままに維持し、`eprintln!` を採用する

### 採用しなかった代替案

- **内部状態を「壊れた」フラグでマークし、次回 `start` で `Err(...)` を返す**: 公開 API の観察可能挙動 (現状は `Ok(())` を返す経路) の後方互換性を崩すことになり、本 issue のスコープ (内部状態破壊の可視化) を超える。`stop` 後の panic 状態を正しくエラー化したいなら別 issue (change カテゴリ) として議論する
- **`std::panic::catch_unwind` で `capture_thread_func` 内のユーザ callback を保護する**: 別 issue (FFI コールバック panic 保護) で扱う範疇。本 issue では扱わない

### 2. `start` の `callback.take()` を判定の後に移す (防御的修正)

`take()` を判定後に置くことで、`Ok(())` で早期 return する経路では `self.callback` を一切変更しない契約を型レベルで明示する。`expect()` を新規導入せずに、`if let Some(...)` パターンで取り出す。

```rust
pub fn start(&mut self) -> Result<()> {
    let session = self.session.as_ref().ok_or(Error::SessionStartFailed)?;
    if self.running.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(callback) = self.callback.take() else {
        return Ok(());
    };
    // ...以降は変更なし...
}
```

`let session = ...` は元の位置を維持 (`Session` がない場合は `Err(Error::SessionStartFailed)` で抜ける本来の挙動を維持)。`running` 判定を先に置き、続けて `callback` の取り出しを試みる。両方とも `Ok(())` で抜ける契約。

### 3. `src/capture.rs::VideoCapture::start` / `stop` の doc 補強

`VideoCapture::start` および `VideoCapture::stop` の doc コメントに、キャプチャスレッドが panic した場合の挙動 (panic 直後の再 `start` は `Ok(())` を返すがキャプチャは再開しない、原因は `stop` 時に stderr へ出力されるログから判別できる) を追記する。文面は実装時に既存 doc の文脈に合わせて記述する。`MfCaptureImpl::start` / `stop` 側の doc コメントは触らない (バックエンド内部実装の doc であり、公開挙動は `VideoCapture` の doc で一元化する)。

`src/lib.rs` のクレートドキュメントは本 issue では触らない (panic 全体の方針は別 issue で扱う)。

## 影響範囲

- `src/capture_mf.rs`: `MfCaptureImpl::start` の判定順序変更、`MfCaptureImpl::stop` の `handle.join() == Err` 経路に `eprintln!` 追加
- `src/capture.rs`: `VideoCapture::start` / `stop` の doc コメント追記
- `src/lib.rs`, `src/types.rs`, `src/device_mf.rs`, `src/error.rs`: 変更なし
- 公開 API: シグネチャ・戻り値・エラーバリアントとも不変
- `Drop for MfCaptureImpl`: 内部で `self.stop()` を呼ぶため、Drop 経由でも本 issue で追加する `eprintln!` ログが流れうる。仕様として許容する (Drop 中の panic を可視化するメリットの方が大きい)
- 0021 (`MfCaptureImpl::new` の修正) および FFI コールバック panic 保護の別 issue (`process_sample` の修正) と同じ `src/capture_mf.rs` を触るが、変更箇所は別関数のためマージ順序の制約はない。コンフリクトが起きた場合は手動でマージする。FFI コールバック panic 保護が先にマージされても、本 issue の `eprintln!` ログは将来別の panic 経路が増えた場合の診断手段として残す

## 完了条件

- `MfCaptureImpl::stop` の `handle.join() == Err` 経路で `eprintln!` でログ出力すること (英語、メッセージ案: `"capture thread panicked; callback is permanently lost and capture cannot be restarted"`)
- `MfCaptureImpl::start` の `callback.take()` 呼び出しを `running` 判定の後に移し、`let Some(callback) = self.callback.take() else { return Ok(()); };` パターンで取り出すこと
- 既存の冪等動作 (`running` 中の再 `start` が `Ok(())` を返す、callback 不在時の再 `start` が `Ok(())` を返す) を維持すること
- `VideoCapture::start` および `VideoCapture::stop` の doc コメントに panic 時の挙動を追記すること (例: 「キャプチャスレッドが panic した場合、`stop` 時に stderr へエラーログを出力する。その後の再 `start` は `Ok(())` を返すがキャプチャは再開しない」旨を `start` 側に、「キャプチャスレッドが panic した場合は stderr にエラーログを出力する」旨を `stop` 側に追記する)
- `cargo test --workspace` および Windows 環境での `cargo test --workspace -- --ignored` が通る (新規テスト追加は本 issue では行わない。`handle.join() == Err` 経路の再現には `MfCaptureImpl` を実機セッションつきで構築する必要があり、現リポジトリの `tests/test_capture.rs` 構造に乗らない。closed/0010, closed/0018, 0021 と同じく分岐網羅はコードレビューで担保する)
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] Windows でキャプチャスレッド panic 後の VideoCapture 再 start が黙って no-op になる問題を stderr ログで可視化する`)。担当者行 (`- @<github-id>`) を含める。防御的修正である `start` の判定順序変更は外部から観察できない内部実装変更のため、CHANGES.md には記載しない

## pending 理由

- `eprintln!` による stderr 出力で十分か、それとも `log` クレート等のログライブラリ依存を追加するか、設計判断に議論の余地があるため pending とする

## reopened にした理由

案 B を採用して実装する。キャプチャスレッド panic 後は `eprintln!` に加え、再 `start` で `Error::CaptureFaulted` を返す。`log` クレートは依存ゼロ方針のため採用しない。`catch_unwind` は採用しない (0023 Won't Fix)。

## 解決方法

案 B で実装した。`catch_unwind` は採用せず (0023 Won't Fix)、Windows Media Foundation ではパニック後の再 `start` をエラー化する。

### 実装内容

1. `Error::CaptureFaulted` を追加し、キャプチャスレッド panic 後の再 `start` で返す
2. `MfCaptureImpl::start` で `running` 判定を `callback.take()` より先にし、callback 不在時は `Err(CaptureFaulted)` を返す
3. `MfCaptureImpl::stop` で `handle.join() == Err` のとき英語の `eprintln!` ログを出す
4. `VideoCapture::start` / `stop` の rustdoc、`lib.rs`、README に panic 契約と `CaptureFaulted` を明記する
5. `CHANGES.md` の `## develop` に `[CHANGE]` エントリを追加する

`log` クレートは依存ゼロ方針のため追加していない。
