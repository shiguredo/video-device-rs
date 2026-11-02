# Windows で COM が STA 初期化済みのスレッドから MF API を呼ぶと必ず ComInitFailed になる

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-windows-mf-sta-com-init
- Polished: {YYYY-MM-DD}

## 目的

`src/types.rs` の `CoInitGuard::new` は `CoInitializeEx(None, COINIT_MULTITHREADED)` を呼び、`is_err()` なら `Error::ComInitFailed` を返す。スレッドが既に STA（シングルスレッドアパートメント）で初期化されている場合、`CoInitializeEx` は `RPC_E_CHANGED_MODE` を返すため、`VideoCapture::new` / `VideoDeviceList::enumerate` は必ず失敗する。

WinForms / WPF などの UI スレッド、OLE を初期化する GUI アプリのメインスレッドは STA であることが多く、そのようなアプリでは MF バックエンドを一切使えない。Media Foundation 自体は MTA を要求するため STA で成功させることはできないが、現状は `COM initialization failed` という原因の分からないエラーだけが返る。

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

`RPC_E_CHANGED_MODE`（`0x80010106`）も `is_err()` に含まれる。

## 設計方針

- `RPC_E_CHANGED_MODE` を別の `Error` variant またはメッセージとして区別し、利用者へ「MTA で初期化されたスレッドで構築すること」を伝える
- または、専用の MTA スレッドで COM / MF を初期化する設計へ変更する
- 少なくとも公開 doc に Windows のスレッド要件を明記する

## 完了条件

- STA 初期化済みのスレッドから構築した場合、原因を特定できるエラーが返る
- MTA で初期化したスレッドからは既存どおり動作する
- スレッド要件が `VideoCapture` / `VideoDeviceList` の doc に明記されている
- Windows 実機または cross-target の check でコンパイルが通る

## 解決方法

{完了時に記入}
