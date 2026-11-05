# Windows で COM が STA 初期化済みのスレッドから MF API を呼ぶと必ず ComInitFailed になる

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-windows-mf-sta-com-init
- Polished: 2026-11-05

## 目的

`src/types.rs` の `CoInitGuard::new` は `CoInitializeEx(None, COINIT_MULTITHREADED)` を呼び、`is_err()` なら `Error::ComInitFailed` を返す。スレッドが既に STA（シングルスレッドアパートメント）で初期化されている場合、`CoInitializeEx` は `RPC_E_CHANGED_MODE` を返すため、`VideoCapture::new` / `new_mf` / `VideoDeviceList::enumerate` / `enumerate_mf` は必ず失敗する。

WinForms / WPF などの UI スレッド、OLE を初期化する GUI アプリのメインスレッドは STA であることが多く、そのようなアプリでは MF バックエンドを一切使えない。Media Foundation は MTA スレッドでの利用が推奨されており、STA スレッドからの利用も可能ではあるが注意が必要である（Microsoft Learn の "Media Foundation and COM" を参照）。本ライブラリは `CoInitializeEx(COINIT_MULTITHREADED)` を必須としているため、STA で初期化済みのスレッドでは構築できない。現状は `COM initialization failed` という原因の分からないエラーだけが返る。

## 優先度根拠

- Medium。Windows GUI アプリの主要なスレッド構成で動作しない
- エラーメッセージから原因（MTA スレッドが必要）を特定できない
- `/review-code` の指摘として確認済み

## 現状

```rust
let result = unsafe {
    windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED)
};
if result.is_err() {
    return Err(crate::Error::ComInitFailed);
}
```

`RPC_E_CHANGED_MODE`（`0x80010106`）も `is_err()` に含まれる。`src/error.rs` の `Error::ComInitFailed` の `Display` は `"COM initialization failed"` であり、原因が分からない。

## 設計方針

- `Error::ComInitFailed` の `Display` を、呼び出し元スレッドを `COINIT_MULTITHREADED` で初期化する必要があることが分かる文言に変更する。例えば `"COM initialization failed: this library requires a thread initialized with COINIT_MULTITHREADED"` とする。新しい `Error` variant は追加しない。`Error` は `#[non_exhaustive]` ではないため variant の追加は公開 API の破壊的変更になり、`pbt/tests/prop_error.rs` の全 variant 生成と `skills/shiguredo-video-device/SKILL.md` のエラー表も更新が必要になる。同じ組織の `shiguredo_audio_device` も `Error::ComInitFailed` のメッセージで原因を示す方針を採っている。
- `src/capture.rs` の `VideoCapture::new` / `new_mf` と `src/device.rs` の `VideoDeviceList::enumerate` / `enumerate_mf` の rustdoc に、Windows では呼び出し元スレッドを `COINIT_MULTITHREADED` で初期化する必要があること、STA で初期化済みの場合は `Error::ComInitFailed` を返すことを明記する。`README.md` の「キャプチャのライフサイクル」にある Windows の記述にも同じ要件を追記する。`skills/shiguredo-video-device/SKILL.md` の Windows 節は既に同じ要件を記載しているため、記述が新しいメッセージと矛盾しないことだけ確認する。
- 専用の MTA スレッドで COM / MF を初期化する設計変更は、`VideoCapture` の `!Send` と既存のキャプチャスレッド設計を大きく変えるため、本 issue では採用しない。別 issue で必要になったときに検討する。
- 本 issue は呼び出し元スレッドの COM 初期化失敗を対象とする。`src/capture_mf.rs` の `capture_thread_func` 側の COM 初期化失敗（`running` が true のまま残る問題）は別 issue で扱う。

## 変更対象

- `src/error.rs`: `Error::ComInitFailed` の `Display` を原因が分かる文言に変更する
- `src/types.rs`: `CoInitGuard::new` の doc コメントに、STA で初期化済みのスレッドでは `RPC_E_CHANGED_MODE` により失敗することを追記する。`#[cfg(all(test, enable_mf))]` モジュールに、STA 初期化済みのスレッドから `VideoCapture::new_mf` / `VideoDeviceList::enumerate_mf` が `Err(Error::ComInitFailed)` を返し、`Display` に `COINIT_MULTITHREADED` が含まれることを確認する単体テストを追加する
- `src/capture.rs`: `VideoCapture::new` / `new_mf` の rustdoc に Windows の MTA 要件を追記する
- `src/device.rs`: `VideoDeviceList::enumerate` / `enumerate_mf` の rustdoc に Windows の MTA 要件を追記する
- `README.md`: 「キャプチャのライフサイクル」の Windows の記述に MTA 要件を追記する
- `CHANGES.md`: `## develop` に `[FIX]` エントリを追加する

## 完了条件

- STA 初期化済みのスレッドから `VideoCapture::new_mf` / `VideoDeviceList::enumerate_mf` を呼ぶと、`Error::ComInitFailed` が返り、その `Display` に `COINIT_MULTITHREADED`（MTA）が必要であることが含まれる
- MTA で初期化したスレッドからは既存どおり動作する（`CoInitializeEx` が `S_FALSE` を返す場合も `CoUninitialize` が対で呼ばれる）
- スレッド要件が `VideoCapture::new` / `new_mf` と `VideoDeviceList::enumerate` / `enumerate_mf` の rustdoc、および `README.md` の「キャプチャのライフサイクル」の Windows の記述に明記されている
- Windows の単体テストで、STA 初期化済みのスレッドから `VideoCapture::new_mf` / `VideoDeviceList::enumerate_mf` が `Err(Error::ComInitFailed)` を返し、`Display` に `COINIT_MULTITHREADED` が含まれることを確認する
- Windows 実機または cross-target の check でコンパイルが通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
