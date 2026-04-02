# Windows: デバイス列挙・キャプチャの残課題（COM、`ReadSample`、コールバックと `join`）

Created: 2026-04-02  
Model: Composer 1  
Completed: 2026-04-02

## 背景・重複確認

- `issues/closed/0009` で **`VideoCapture::new` の `MFStartup` ガード**、`activate_device` の **`CoTaskMemActivateArrayGuard`**、`process_sample` の **`Lock` / 幅高さ / バッファ長** などが扱われている。
- 本 issue は **列挙専用の `enumerate_devices_internal`**、**`get_device_formats` の COM 順**、**`ReadSample` 失敗ループ**、**`stop` の `join`**、**YUY2 の `stride` 乗算**に絞る。

## 現状コード（調査結果）

### A. `enumerate_devices_internal`（`src/device_windows.rs` 約 182〜197 行）

```text
CoInitializeEx → MFStartup → enumerate_devices_impl → MFShutdown → result
```

- **`MFStartup` が失敗**（188 行 `?`）すると **`MFShutdown`（193 行）に到達しない**。  
  - **MF の仕様**: `MFStartup` 失敗時は **カウントを上げていない**ため **`MFShutdown` を呼ばない**のが正しい場合がある。**MSDN / MF ドキュメントで確認**し、**コメントで意図を残す**。
- **`CoInitializeEx` に対する `CoUninitialize`** は **成功・失敗どちらにもない**。**スレッドあたりの参照カウント**の観点で、**列挙関数を呼ぶスレッド**がアプリ全体の COM 方針と合うか検討。**対称にするなら** `unsafe` ブロックを **`defer` 風**にするか、**RAII 構造体**で `CoUninitialize` を **`MFShutdown` 後**に呼ぶ（**既に `CoInitialize` 済みスレッド**では `S_FALSE` 等になる挙動を確認）。

### B. `get_device_formats`（同ファイル 約 111〜179 行）

**採用確定（レビュー反映）**: **`Shutdown()` と COM の `Drop`（`Release` 相当）を「二重解放」として短絡する書き方は不採用**。`IMFMediaSource::Shutdown()` は **API メソッド**であり、`Drop` で走るのは **参照カウントの解放**であり、**同義ではない**。**バグ修正として W1 を立てるなら**、MSDN 等で **`Shutdown` 前にソースリーダーを解放すべき**などの **契約違反・実害**を**引用可能な根拠**で示す。本 issue では **W1 を「可読性・意図の明示」に寄せる**。

- 122〜128 行: `reader` 作成失敗時は **`source.Shutdown()`** して return。
- 175 行: ブロック終了前に **`source.Shutdown()`** を明示呼び出し。**`reader` はまだスコープ内**（114〜176 行）。ブロック終了時、**`reader` → `source` の順で `Drop`**（宣言の逆順）が走る想定。
- **W1 の位置づけ（推奨）**: **`reader` を先にスコープ外にする**（明示 `drop(reader)` または内側 `{}`）**あと**に **`source.Shutdown()`** を呼ぶ、と**読み手が順序を追いやすくする**リファクタ。**必須のバグ修正と断定しない**（実害の根拠を別途得たときは issue 本文またはコメントに追記する）。

### C. `capture_thread_func`（`src/capture_windows.rs` 約 426〜442 行）

- `ReadSample` が **`Err` のとき `continue` のみ**。**スリープなし** → **連続失敗で CPU 占有**。
- **対策案**: `result.is_err()` のとき **`std::thread::sleep`**（例: 1ms）を入れる、または **一定回数でループを抜けて `running` を false にする**（**ポリシー要決定**）。

### D. `stop` と `join`（同ファイル 176〜185 行）

- **`capture_thread` 上**で動く `process_sample` 内の **ユーザーコールバック**（601 行付近）から **`VideoCapture::stop`** を呼ぶと、**同じスレッドが `join` 自身**し得る（**デッドロック**）。

### E. YUY2 の `stride`（同ファイル 約 573〜588 行）

- `yuy2_packed_frame_bytes_win`（468〜470 行）は **`width.checked_mul(2)`**。
- **582 行** `let stride = width * 2;` は **checked ではない**。**`width` が大きいと `i32` オーバーフロー**の理論的余地。

## 提案する実装（タスク分割）

| ID | 内容 | ファイル |
|----|------|----------|
| W1 | `get_device_formats` で **`reader` 解放を先に明示**し、**`source.Shutdown()` の意図する順序を読みやすくする**（**可読性**。**二重解放対策と断定しない**） | `device_windows.rs` |
| W2 | `enumerate_devices_internal` の **MF / COM の成否と `MFShutdown` / `CoUninitialize` の対応**を MSDN 確認のうえ **コメント + 必要なら RAII** | `device_windows.rs` |
| W3 | `ReadSample` 失敗時の **スリープまたはバックオフ** | `capture_windows.rs` |
| W4 | **`lib.rs` または `VideoCapture` の rustdoc** に「**コールバック内から `stop` を呼ばない**」を明記（Unix の `pthread_join` と同趣旨） | `lib.rs` または `capture_windows.rs` |
| W5 | YUY2 の **`stride` を `width.checked_mul(2)`** 等に統一。失敗時はフレーム破棄 | `capture_windows.rs` |

## テスト・検証

- **Windows** で `cargo test`（既存）+ **手動**: デバイス列挙、キャプチャ開始/停止。
- **`get_device_formats` 変更後**: 列挙が **空にならない**こと（回帰）。

## 完了条件（チェックリスト）

- [ ] W1〜W5 のうち **採用した項目**が実装または**意図的に不要**とドキュメント化されている。
- [ ] `0009` の修正内容と**矛盾しない**（`activate_device` / `process_sample` を**再び壊さない**）。

## 依存関係

- **`closed/0009`** を読んでから着手（上書き競合に注意）。

## 関連ファイル一覧

| ファイル |
|----------|
| `src/device_windows.rs` |
| `src/capture_windows.rs` |
| `src/lib.rs`（ドキュメントのみの場合） |

## 解決方法

### W1: `get_device_formats` の解放順（可読性）

**ファイル**: `src/device_windows.rs` の `get_device_formats`。

メディアタイプ列挙ループの後、**`IMFSourceReader` を先に `drop(reader)`** し、その後 **`source.Shutdown()`** を呼ぶ（175〜177 行）。issue の「`reader` を先にスコープ外にする」方針どおり。`Shutdown` と `Release`（Drop）を同一視しない前提は維持し、**読み手が順序を追いやすくする**リファクタに留めた。

### W2: 列挙パスの MF / COM とコメント

**ファイル**: `src/device_windows.rs` の `enumerate_devices_internal`。

- **`CoInitializeEx` 後に `CoUninitialize` を呼ばない**理由を日本語コメントで記載（186 行付近）。スレッドの参照カウントとアプリ方針に合わせる旨。
- **`MFStartup` が失敗したときは `MFShutdown` を呼ばない**ことを、MSDN の初期化契約に従う旨でコメント（189 行付近）。
- 成功時のみ **`enumerate_devices_impl` の後に `MFShutdown`**（194〜195 行）。

### W3: `ReadSample` 失敗時の CPU 占有

**ファイル**: `src/capture_windows.rs` の `capture_thread_func`。

`ReadSample` が `Err` のとき、`thread::sleep(Duration::from_millis(1))` してから `continue`（444〜447 行）。連続失敗でビジーループにならないようにした。

### W4: コールバック内 `stop` の禁止

**ファイル**: `src/capture_windows.rs` の `VideoCapture` 構造体の rustdoc（67〜69 行）。

**キャプチャコールバック内から `stop` を呼ばない**こと、**同じスレッドが `join` 自身しデッドロックしうる**ことを明記。`src/lib.rs` のクレートドキュメント（14〜15 行）とも整合。

### W5: YUY2 の `stride` と `required` バイト数

**ファイル**: `src/capture_windows.rs`。

- **`yuy2_packed_frame_bytes_win`**: `width.checked_mul(2)` でストライドを得てから高さ分を掛ける（474〜476 行）。オーバーフロー時は `None`。
- **`process_sample` の YUY2 分岐**: `required` 算出失敗時は `Unlock` して return（579〜583 行）。`stride` は **`width.checked_mul(2)`**（588〜591 行）。失敗時はフレームを組み立てず return。

### `closed/0009` との関係

`0009` で `process_sample` のバッファ検証・MF ガード等が入った後でも、本 issue の列挙・`ReadSample`・ドキュメント・YUY2 `checked_mul` は **別コミット相当の追加対応**として両立している。
