# `device_windows` の列挙バッファを `Drop` ガードで `CoTaskMemFree` する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

**メモリリーク**の除去。`MFEnumDeviceSources` が返す配列は `CoTaskMemFree` 必須。ループ内（`get_device_formats` など）で**パニック**した場合、現状の末尾 `CoTaskMemFree` に到達しないとリークする。

## 現状

- ファイル: `src/device_windows.rs`
- 関数: `enumerate_devices_impl`（約 185〜238 行）
- `count > 0 && !devices_ptr.is_null()` のブロック内で `from_raw_parts` → `for` ループ → 末尾で `CoTaskMemFree` のみ。

## 修正方針

1. `src/capture_windows.rs` に既にある **`CoTaskMemActivateArrayGuard`**（約 51〜64 行）と**同一パターン**を `device_windows.rs` に置く。  
   - 共通化する場合は `src` 直下に小さなモジュール（例: `win_cotaskmem.rs`）に切り出して両方から `use` してよい。重複コピーでも可。
2. `MFEnumDeviceSources` 成功かつ `devices_ptr` が非 null の直後に `let _guard = CoTaskMemActivateArrayGuard { ptr: devices_ptr };` を置き、**手動の `CoTaskMemFree`（約 233〜234 行）は削除**する。
3. `from_raw_parts` には引き続き `devices_ptr` を使う（ガードと同じ生ポインタ）。関数終了時にガードの `Drop` で必ず解放。

## 注意

- `CoTaskMemFree` は**一度だけ**（ガードの `Drop` に一本化）。二重解放しないこと。
- `activate_device`（`capture_windows.rs`）と同じ `Option<IMFActivate>` の配列であること。

## 完了条件（チェックリスト）

- [x] 列挙バッファが、正常終了・早期 return・パニックのいずれでもリークしない構造になっている。
- [x] `cargo check --target x86_64-pc-windows-msvc` が通る。

## 完了条件の検証

2026-04-02 にソースとビルドで確認した。

- `device_windows.rs`: `MFEnumDeviceSources` 成功後に `CoTaskMemActivateArrayGuard` を束縛し、手動 `CoTaskMemFree` は削除済み（約 223〜224 行）。`enumerate_devices_impl` を通常のパニックアンワインドで抜ける場合、`Drop` で `CoTaskMemFree` が一度呼ばれる構造。
- `cargo check --target x86_64-pc-windows-msvc` を実行し成功。
