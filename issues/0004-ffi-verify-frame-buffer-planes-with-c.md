# Unix フレームコールバックのプレーン長を C 実装と突き合わせる

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

`from_raw_parts` に渡す長さが**実バッファ境界を超えない**ことの検証。**セグフォ／未定義動作**を防ぐ。

## スコープの切り方（重要）

- **本 issue の主成果物は macOS I420**（`src/video_c.m` の連結 UV と Rust の `i420_plane_sizes` の式合わせ）。ここを最優先で閉じる。
- **V4L2** は「Rust と一言で一致した」とコメントするだけでは**安全性の証明にならない**。`src/video_v4l2.c` 554〜561 行付近では NV12 の `y_size` を `width * height` とし、コールバックの stride に **`session->width` を渡している**が、**`bytesperline`（実ストライド）は見ていない**。パディング付きバッファでは境界と乖離しうる。対応するなら **C 側で実ストライドを渡す変更**、または **不一致リスクを日本語コメントで明示**する別タスクとし、本 issue の完了条件に「V4L2 と Rust が数学的に一致した」とまでは書かない。
- **PipeWire** は `on_process` 内の stride とネゴシエート済み解像度の関係を、必要なら**別コメントまたは follow-up** で扱う。本 issue の必須チェックリストには含めない。

## Rust 側の計算（基準）

- ファイル: `src/capture.rs`
- 関数: `nv12_plane_sizes`, `i420_plane_sizes`, `yuy2_packed_frame_bytes`, および `frame_callback` 内での使用。

### I420（macOS・重点）

`src/video_c.m`（約 78〜94 行）:

- `chromaHeight = (height + 1) / 2`
- `strideUV = max(strideU, strideV)`
- 連結 UV バッファ `uvBuffer` のサイズ: `uvSize = strideUV * chromaHeight * 2`
- コールバックに渡す `stride_uv` は `(int)strideUV`、`height` はフレーム高さ。

Rust `i420_plane_sizes` は Y を `stride * height`、UV（連結 U+V）を **`stride_uv * chromaHeight * 2`**（`chromaHeight = (height + 1) / 2`、usize 上の切り捨て整合）とし、C の `uvSize` と一致させる。**当初メモにあった「UV を `stride_uv * height` とする」案は、奇数高さで C と不一致になるため不採用**（採用結果は **0008** の実装記録にも書いた）。

**検証タスク**:

1. 偶数・奇数 `height` について、C の `uvSize` と Rust が `from_raw_parts(uv_data, uv_size)` に使う `uv_size` が**同じバイト数**になるか式で示す。  
2. 一致しない場合は **Rust の式を C に合わせる**か、C の割り当てを変えない前提で Rust を修正する。  
3. 結果を **`src/capture.rs` の `i420_plane_sizes` 付近に日本語コメント**で根拠付きで残す。

### V4L2（参考・本 issue の必須成果ではない）

- 現状の C は上記のとおり `width` ベース。境界保証を強める変更は**別 issue** または本 issue の拡張として切り出す。

## 完了条件（チェックリスト）

- [ ] macOS I420 について、C の `uvSize` と Rust の UV バイト長が式レベルで突き合わせられ、コメントまたは文書に残っている。
- [ ] V4L2 を触る場合は、**bytesperline 未使用の限界**をコメントに書くか、コード変更まで含めるかを明示したうえで対応する（無理に「一致した」と書かない）。

## 変更履歴について

- 現状リポジトリに **`CHANGES.md` は無い**。変更履歴を別途導入する運用になった場合は `AGENTS.md` に従う。**本 issue の完了条件に `CHANGES.md` の追記は含めない**（導入済みになった時点でルールに合わせる）。

## 参考

- `src/video_c.h` の `FrameCallback` 引数説明（26 行付近）。
- `src/video_v4l2.c`: NV12 コールバック（554 行付近）。
