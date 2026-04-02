# Windows `capture_windows.rs` の MF 初期化・列挙バッファ・サンプルバッファを防御する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

1. **`MFStartup` 成功後**に `Err` したとき **`MFShutdown` が呼ばれず** MF の参照が残る問題の解消。  
2. **`activate_device` 内**で `MFEnumDeviceSources` が成功したあと **`DeviceNotFound` や `ActivateObject` 失敗**で return する経路でも **`CoTaskMemFree` が走らず列挙配列がリーク**する問題の解消。  
3. **`process_sample` 内**で `Lock` 後の **null データポインタ**、**幅・高さ非正**、**バッファ長不足**、**YUY2 でバッファ全体を誤って渡す**ことの防止。

## 現状調査（この issue 作成時点の `src/capture_windows.rs`）

### A. `VideoCapture::new`（約 37〜87 行）

- 47 行: `MFStartup` 成功後、`activate_device` / `create_source_reader` 等で `?` すると **`MFShutdown` なしで抜ける**。`Drop` の `MFShutdown` は `VideoCapture` が構築された場合のみ。

### B. `activate_device`（約 160〜218 行）

- 178 行: `MFEnumDeviceSources` 成功後、`count > 0 && devices_ptr` でスライスを作る。  
- **181〜182 行**: `count == 0` 等で `Err(DeviceNotFound)` → この経路では `devices_ptr` 未使用なら問題小。  
- **188〜200 行・203〜207 行**: `ok_or(DeviceNotFound)?` で **`devices_ptr` を解放せず return** → **リーク**。  
- **210〜212 行**: `ActivateObject` 失敗でも同様。  
- **214〜215 行**: 成功時のみ `CoTaskMemFree`。上記失敗経路では **手動 Free に到達しない**。

### C. `process_sample`（約 404〜503 行）

- 423〜434: `Lock` 成功後、**`data_ptr` が null** の分岐が無い（434 行で `from_raw_parts`）。  
- **幅・高さ非正**のチェックが無い（437 行以降で `width * height` を使用）。  
- **NV12**（437〜456）: `y_size` 未満チェックのみ。連続バッファ全体として必要な **Y+UV 分**まで足りるかの判定は、再実装では `y_size + y_size/2` 等で `required` を取り `data.len() < required` で return する形が望ましい。  
- **I420**: 同様に UV 分を含めた `required` を検討。  
- **YUY2**（479〜491）: `data` に**バッファ全体**（`current_length` 分）を渡している。`stride * height` 未満なら危険。**`&data[..required]`** に切る。

## 実装の置き場所

### 1. ガード用 `struct` と `Drop`

- ファイル先頭付近（**`SendPtr` の後、`SessionData` の前**、約 21 行後あたりが目安）に以下を追加する。  
  - `MfShutdownGuard { active: bool }` … `Drop` で `active` が true なら `MFShutdown()`。  
  - `CoTaskMemActivateArrayGuard { ptr: *mut Option<IMFActivate> }` … `Drop` で `ptr` が非 null なら `CoTaskMemFree(Some(ptr as *const _))`（既存 215 行と同型の呼び出し）。

### 2. `VideoCapture::new`（`unsafe` ブロック内）

- **47 行** `MFStartup` 成功の直後に `let mut mf_guard = MfShutdownGuard::new();`  
- **`Ok(Self { ... })` の直前**（80 行前）に `mf_guard.active = false;`（成功時は `Drop` で `MFShutdown` しない）。

### 3. `activate_device`

- **185 行** `device_slice` を作る**前**に、`count > 0 && !devices_ptr.is_null()` が真のとき  
  `let _devices_guard = CoTaskMemActivateArrayGuard { ptr: devices_ptr };`  
- **214〜215 行の手動 `CoTaskMemFree` は削除**（ガードの `Drop` に任せる）。  
- `from_raw_parts(devices_ptr, …)` は従来どおり `devices_ptr` でよい（ガードと同じポインタ）。

### 4. `process_sample`

- `Lock` 成功後、`data_ptr.is_null()` なら `Unlock` して return。  
- `width <= 0 || height <= 0` なら `Unlock` して return。  
- NV12 / I420: `checked_mul` 等でフレームに必要なバイト数 `required` を算出し、`data.len() < required` なら `Unlock` して return。スライスは常に `data` の**先頭から `required` バイト**に限定。  
- YUY2: `width.checked_mul(2)` と `required = stride * height`（`usize` で `checked_mul`）、`data.len() < required` なら return。`VideoFrame.data` は `&data[..required]`。

## 関連 issue

- **0003**: `src/device_windows.rs` の `enumerate_devices_impl`（**別ファイル**。本 issue は **`capture_windows` の `activate_device` のみ**）。

## 完了条件（チェックリスト）

- [x] `MfShutdownGuard` が `new` のエラー経路で `MFShutdown` する。  
- [x] `activate_device` の**全** `Err` 経路で列挙配列が解放される（ガードで確認）。  
- [x] `process_sample` が null ポインタ・非正の幅・高さ・バッファ不足・YUY2 のスライス範囲を扱う。  
- [x] `cargo check --target x86_64-pc-windows-msvc` が通る。

## 完了条件の検証

2026-04-02 に `capture_windows.rs` とクロスコンパイルで確認した。

- `MfShutdownGuard` が `MFStartup` 直後に束縛され、構築成功時のみ `active = false`（約 97 行・約 130 行）。
- `activate_device` で `CoTaskMemActivateArrayGuard` を列挙直後に束縛。
- `process_sample` で null ポインタ・非正の幅・高さ・`required` 不足を拒否し、YUY2 は `&data[..required]`。
- `cargo check --target x86_64-pc-windows-msvc` 成功。

## 検討結果

- **調査**: 現行コードは `MFStartup` 後の失敗、`activate_device` の複数 `Err`、YUY2 のフルバッファ渡しに欠陥がある。  
- **本 issue だけで修正可能**: はい。**`src/capture_windows.rs` のみ**で完結（他ファイルは触らない）。  
- **変更履歴**: リポジトリに `CHANGES.md` が無い場合は、プロジェクト運用に従い別途追記するか省略。

## 問題解決

### 問題だったこと

1. **`MFStartup` 成功後に `Err` で return** すると、`VideoCapture` が構築されず **`Drop` の `MFShutdown` も走らない**ため、MF の参照カウントが残りうる。
2. **`activate_device`**: `MFEnumDeviceSources` が成功したあと、`DeviceNotFound` や `ActivateObject` 失敗で抜けると **`CoTaskMemFree` が呼ばれず**列挙配列がリークする（成功時だけ手動 `CoTaskMemFree` していた）。
3. **`process_sample`**: `Lock` 後に null データ・非正の幅・高さ・**バッファ長不足**・YUY2 で **バッファ全体**を `VideoFrame.data` に渡すと、`from_raw_parts` やスライスが**境界外**になりうる。

### どう解決したか

1. `MFStartup` 直後に `MfShutdownGuard` を置き、**`VideoCapture` 構築が成功したときだけ** `active = false` にして `Drop` で `MFShutdown` しない。それ以外の `?` 経路ではガードの `Drop` で `MFShutdown` する。
2. 列挙ポインタ取得後すぐ `CoTaskMemActivateArrayGuard` を束縛し、**成功・失敗を問わず** `CoTaskMemFree` する（手動 `CoTaskMemFree` は削除）。
3. `data_ptr.is_null()`、`width` / `height` 非正を拒否。NV12/I420 は **Y+UV 相当の必要バイト数**（Y + Y/2）を `required` とし `data.len() < required` なら return。YUY2 は `stride * height` を `required` とし **`&data[..required]`** のみ渡す。null/不足時は `Unlock` してから return。

### issue 本文との差異

| 項目 | issue 本文 | 実際の解決 |
|------|------------|------------|
| `process_sample` の `required` 計算 | 本文は `process_sample` **内**で直接計算する想定の記述 | 同じ式だが **`nv12_packed_frame_bytes` / `i420_packed_frame_bytes` / `yuy2_packed_frame_bytes_win`** に**関数として切り出し**、可読性と重複回避にした |
| MF / CoTaskMem | `MfShutdownGuard`・`CoTaskMemActivateArrayGuard` の導入 | **本文どおり** |

**0003**（`device_windows.rs` の列挙）とは別ファイルだが、`CoTaskMemActivateArrayGuard` のパターンは同型である。
