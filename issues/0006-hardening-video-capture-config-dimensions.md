# `VideoCaptureConfig` の幅・高さ・fps の妥当性を検証する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

不正な `width` / `height` / **`fps`** が **Windows の `try_set_format`** などに渡ると、`as u64` で**化けた値**が MF に入り、**未定義動作**につながりうる（間接的なリスク）。  
**PipeWire 経路は除外して検討する**（後述）。

## 現状

- `src/types.rs` の `VideoCaptureConfig`: `width`, `height`, `fps` は `i32`、検証なし。
- Windows: `src/capture_windows.rs` の `try_set_format` で  
  - `frame_size = ((width as u64) << 32) | (height as u64)`（`MF_MT_FRAME_SIZE`）。負の `i32` は `as u64` で下位 32bit が化ける。  
  - `frame_rate = ((fps as u64) << 32) | 1`（`MF_MT_FRAME_RATE`）。**`fps` も同様に `as u64` で詰める**ため、負や不適切な値は化けうる。
- **PipeWire**（`src/video_pipewire.c` の `video_session_create`、約 442〜450 行）: `width <= 0` / `height <= 0` / `fps <= 0` を **640×480×30 に丸める**実装がある。Rust の `VideoCapture::new` で一律に「正の整数以外は Err」とすると、**現行の「不正値は C でデフォルトに置換」という挙動を壊す**（互換性を壊す変更になる）。

## 修正方針（採用時に明示）

**案 A（推奨・破壊を避ける）**

- 検証を **`#[cfg(target_os = "windows")]` の `VideoCapture::new`（または `try_set_format` 手前）に限定**する。**`width` / `height` / `fps` を同じ検証ブロックで扱う**（片方だけ厳しくして `fps` だけ放置しない）。V4L2 単体ビルドでは `video_session_create` に渡す前に同様の検証を入れるかは別判断。
- **PipeWire ビルド**では Rust 側で `width`/`height`/`fps` を拒否しない（C の丸めに任せる）。または issue を分けて「PipeWire も厳格化する」場合は **CHANGES と互換性の説明**を必須にする。

**案 B（共通バリデーション）**

- `VideoCapture::new` 共通で正の整数のみ許可する。**その場合は PipeWire のデフォルト置換をやめるか、C を先に合わせる**など、**互換性を壊す変更**であることをリリースノートに書く。

## エラー型（案 A を採る場合の例）

- `src/error.rs` に `InvalidCaptureConfig(&'static str)` 等を追加。`Display` は**英語**。

## 完了条件（チェックリスト）

- [ ] 採用した案（A または B）が issue または PR 説明に一文ある。
- [ ] **Windows** で負またはゼロの **幅・高さ**がプロジェクトが選んだルールどおり拒否される（案 A の場合）。
- [ ] **Windows** で **`fps`**（例: `fps <= 0` や負の扱い）が、**幅・高さと同じ方針**で拒否されるか、または意図的に許容するなら **`VideoCaptureConfig` の doc と実装の両方**にその旨がある（片方だけにしない）。
- [ ] PipeWire の丸め挙動を壊していない、または案 B で意図的に壊す旨が文書化されている。
- [ ] `VideoCaptureConfig` の doc に、**日本語で 1〜2 文**、幅・高さ・**fps** の前提（正の整数が必要か、PipeWire は C 側で丸めるか等）を書く。

## 変更履歴について

- 現状リポジトリに **`CHANGES.md` は無い**。導入後は `AGENTS.md` に従う。**本 issue の完了条件に `CHANGES.md` の追記は含めない**。

## 検討結果

- **採用**: レビュー指摘どおり、タイトル・目的に `fps` があるのに完了条件に無かった**曖昧さを解消**した。`try_set_format` 内の `MF_MT_FRAME_RATE` への `fps as u64` を現状に明記し、完了条件に **Windows での `fps` の扱い**（幅・高さと同じ検証ブロック、または doc と実装の両立）を追加した。

## 実装記録

- **案 A** を採用。検証は `src/capture_windows.rs` 内の非公開関数 `validate_capture_config_for_windows` とし、`VideoCapture::new` の**先頭**（`MFStartup` より前）で `width` / `height` / `fps` を一括チェックした。issue 本文の「`try_set_format` 手前」より**前**で拒否することで、不正値が MF に渡らないようにした。
- `src/error.rs` に `Error::InvalidCaptureConfig(&'static str)` を追加（本文のエラー型の例と同型）。
- `src/types.rs` の `VideoCaptureConfig` に、Windows と Linux（PipeWire の丸め）の差を日本語 doc で記載した。
