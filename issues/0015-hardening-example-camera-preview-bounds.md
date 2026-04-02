# サンプル `camera_preview` のフレーム境界・チャネル背圧

Created: 2026-04-02  
Model: Composer 1

## なぜこの対応が必要か

`examples/camera_preview.rs` は **お手本**として参照されやすい（`AGENTS.md`）。現状、**ライブラリが返す `VideoFrame` の寸法とスライス長の関係を検証せず**に **`strip_stride` でインデックス**しており、**異常値でパニック**し得る。また **`mpsc` 無制限チャネル**で **メモリ増大**し得る。

## 現状コード（調査結果）

### 参照ファイル

- `examples/camera_preview.rs`
- `Cargo.toml` の `[[example]]` で **`raw-player` feature 必須**

### 問題箇所

| 行付近 | 内容 |
|--------|------|
| 194〜197 | `frame.width as usize` 等。**負の `i32` は `usize` で巨大値**になり、**151〜152 / 157** でパニックし得る |
| 151〜152 | `Cow::Borrowed(&data[..row_bytes * height])`。**`data.len()` 不足**でパニック |
| 209〜218 | I420 の **`uv_data[..u_plane_size]`**。**長さ不足**でパニック |
| 247〜260 | **`mpsc::channel` は無制限**。**`to_owned()` で毎フレーム全コピー** |
| 259 | **`send` 失敗を無視** | 受信側ドロップ後の挙動 |

### `parse_args`（57〜73, 76〜140 行）

- カスタム `WxH` は **`parse::<i32>`** のため **負の解像度**が入り得る。**Linux では C が置き換える**一方、**Windows** は `VideoCapture::new` で **`validate_capture_config_for_windows`** により拒否される（`capture_windows.rs` 78〜82 行）。**サンプル単体で一貫した説明**が欲しい。

## 提案する実装（手順）

### 1. `strip_stride` の防御

- 関数の先頭で **`row_bytes`、`height`、`stride` のオーバーフロー**を **`checked_mul` で必要バイト数**を算出。
- **`data.len()`** と比較し、**不足なら** `Cow::Owned(Vec::new())` を返すか、**呼び出し元で `Err` 相当**にする。**パニックしない**ことを優先。
- または **`strip_stride` を `Result<Cow<...>, &'static str>`** に変更し、`enqueue_video_frame` で `eprintln!` して **`Ok(())`**。

### 2. `enqueue_video_frame` の入口

- **`frame.width <= 0 || frame.height <= 0`** のとき **早期 return**（英語ログ1行）。
- **I420**:  **`uv_data.as_ref().map(|v| v.len())`** と **`u_plane_size` / 全体長**を比較し、**不足なら return**。

### 3. チャネル（方針を決めてから実装）

**採用確定**: コードレビュー指摘 **P2**（キャプチャコールバック内で **`sync_channel` のブロッキング `send` による背圧は採用しない**）を **issue の正として採用**した。

**注意（レビュー反映）**: キャプチャ**コールバック内**で **`sync_channel::send` がブロック**すると、**キャプチャスレッドが詰まり**、取り込み停止・フレーム欠落・停止遅延を**悪化**させる。**「`sync_channel` で背圧」というだけの案は不適切**。

- **採用するなら次のいずれかに方針を絞る**:
  - **`try_send`**: 満杯なら **そのフレームを捨てる**（英語ログ）。**コールバックはブロックしない**。
  - **最新 N 枚だけ保持**（`Mutex` + `VecDeque` 等）し、**古いものを捨てる**（実装は別スレッドに逃がす設計も可）。
  - **別スレッド**が受信専用で処理し、コールバックは **非ブロッキング**にだけ手渡す。
- **無制限 `mpsc` のまま**にする場合は、**メモリ増大のリスク**を **コメントで明示**する（**ブロッキングで背圧をかけない**ことも明記）。

### 4. パース（任意）

- **`parse_resolution`** で **`w > 0 && h > 0`** を要求する。

## テスト・検証

- **手動**: `cargo run --example camera_preview --features raw-player`（環境にカメラがある場合）。
- **ユニットテスト**: `strip_stride` を **`examples` から `tests` に切り出す**のは重いので、**ロジックを小さな `fn` に分けて `#[cfg(test)]`** でテスト（**任意**）。

## 完了条件（チェックリスト）

- [ ] **代表的な不正寸法・短いスライス**でも **`enqueue_video_frame` がパニックしない**。
- [ ] チャネル方針が **コメントまたは実装**で分かる。**キャプチャコールバック内でチャネル送信がブロックしない**（`sync_channel` の**ブロッキング `send` だけ**で背圧、は不可）。
- [ ] 新規コメントは **日本語**、ログは **英語**（`AGENTS.md`）。

## 依存関係

- **なし**（`0013` の Windows ドキュメントと独立）。

## 関連ファイル一覧

| ファイル |
|----------|
| `examples/camera_preview.rs` |
| `Cargo.toml`（feature の説明は変更しない場合も可） |
