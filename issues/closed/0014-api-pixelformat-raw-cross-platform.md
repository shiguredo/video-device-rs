# `PixelFormat::from_raw` / `to_raw` の Windows との API 対称性

Created: 2026-04-02  
Model: Composer 1  
Completed: 2026-04-02

## なぜこの対応が必要か

利用者が **FourCC（`VIDEO_PIXEL_FORMAT_*`）と `PixelFormat` を相互変換**したいとき、**macOS / Linux では `to_raw` / `from_raw` が使える**が、**Windows では `#[cfg]` によりメソッドが存在しない**（`src/types.rs` 32〜52 行）。**クロスプラットフォームのヘルパー**を書きにくい。

## 現状コード（調査結果）

### `src/types.rs`

- **11〜17 行**: `VIDEO_PIXEL_FORMAT_*` 定数は **`cfg(any(target_os = "macos", target_os = "linux"))`**。
- **34〜52 行**: `from_raw` / `to_raw` も同じ `cfg`。

### Windows の実装

- `src/capture_windows.rs` の **`pixel_format_to_guid`**（331〜337 行）で **`PixelFormat` → `GUID`**。
- デバイス列挙は `src/device_windows.rs` の **`guid_to_pixel_format`**（97〜108 行）。

**FourCC と GUID の対応**は **Windows 専用の別経路**になっている。

## 提案する実装（選択肢）

### オプション A（ドキュメントのみ・破壊なし）

1. **`PixelFormat` の rustdoc**（`impl PixelFormat` 上）に以下を書く:
   - **Windows では `to_raw` / `from_raw` は利用できない**（ビルドエラーになる）。
   - **キャプチャ・列挙は `windows` クレートの `GUID` と `PixelFormat` の対応**を内部で行っている。
2. **`VideoCaptureConfig::pixel_format`** の説明に、**プラットフォームごとの意味**を1文追加。

### オプション B（API 追加・後方互換を維持）

1. **Windows でも使える**名前で **FourCC 相当の `u32`** を返す関数を追加する例:
   - `pub fn to_video_pixel_format_value(&self) -> u32`  
   - 実装は **NV12/YUY2/I420 を `0x3231564E` 等で返す**（`video_c.h` と同じ値）。**`Unknown(u32)` はそのまま返す**。
2. **`from_video_pixel_format_value(raw: u32) -> Self`** を **全ターゲット**で提供し、**内部は現状の `from_raw` と同じ match**（Linux/macOS の定数を **Windows でも `cfg` なしで定数定義**するか、`match raw` で数値リテラルを使う）。

**注意**: **オプション B** は **公開 API の追加**になるため **`CHANGES.md` ルール**に従う（プロジェクトに `CHANGES.md` が無い場合は **リリースノート方針**に合わせる）。

### オプション C（GUID 公開・Windows 専用）

- **`PixelFormat` に `#[cfg(windows)] pub fn to_mf_guid(&self) -> GUID`** のようなメソッドを **`capture_windows` から `types` に移す**のは **依存関係が逆転**しやすい。**非推奨**。

## テスト・検証

- **オプション B** の場合: **`cfg(test)`** で **FourCC 値 → `PixelFormat` → 同じ値**のラウンドトリップテストを **Windows でも実行**。

## 完了条件（チェックリスト）

- [ ] **オプション A または B** のいずれかが実装完了。
- [ ] **公開 API を増やした場合**は **CHANGES / 変更履歴ポリシー**に従う。

## 依存関係

- **なし**。

## 関連ファイル一覧

| ファイル | 変更想定 |
|----------|----------|
| `src/types.rs` | 定数の `cfg`、メソッド追加、rustdoc |
| `src/capture_windows.rs` | 重複があれば `pixel_format_to_guid` と共通化 |

## 解決方法

### 方針

issue の **オプション A（ドキュメントのみ・破壊なし）** を採用した。Windows 向けに `from_raw` / `to_raw` を新たに公開する API 追加（オプション B）は行っていない。

### 実装内容

**ファイル**: `src/types.rs`。

1. **`PixelFormat` 型の rustdoc**（19〜22 行付近）に以下を記載した。
   - **Windows** では `to_raw` / `from_raw` は **`#[cfg(any(macos, linux))]` によりビルドに含まれない**（利用するとコンパイルエラーになる）。
   - **列挙・キャプチャ**では Media Foundation の **`GUID`** と `PixelFormat` の対応を **`device_windows.rs` / `capture_windows.rs` 内**で行っている。
2. **定数 `VIDEO_PIXEL_FORMAT_*`** は従来どおり macOS/Linux のみ（11〜17 行）。Windows は FourCC 定数を公開 API としては露出しない。

### 利用者向けの読み方

クロスプラットフォームで FourCC 数値とやり取りする場合は **macOS/Linux では `to_raw` / `from_raw`**、**Windows では内部の GUID 対応に依存せず、本クレートの `VideoCaptureConfig` と列挙結果の `PixelFormat` を使う**形が前提になる、という旨がドキュメントから分かるようにした。
