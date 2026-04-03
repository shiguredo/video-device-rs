# Unix フレームコールバックのプレーン長を C 実装と突き合わせる

Created: 2026-04-02  
Model: Composer 2 Fast  
Completed: 2026-04-02

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

Rust 側の目標は、macOS の `from_raw_parts(uv_data, uv_size)` に渡す `uv_size` が C が `calloc` した `uvSize` と**同じバイト数**になることである（下記「問題解決」）。

**検証タスク**:

1. 偶数・奇数 `height` について、C の `uvSize` と Rust が `from_raw_parts(uv_data, uv_size)` に使う `uv_size` が**同じバイト数**になるか式で示す。  
2. 一致しない場合は **Rust の式を C に合わせる**か、C の割り当てを変えない前提で Rust を修正する。  
3. 結果を **`src/capture.rs` の `i420_plane_sizes` 付近に日本語コメント**で根拠付きで残す。

### V4L2（参考・本 issue の必須成果ではない）

- 現状の C は上記のとおり `width` ベース。境界保証を強める変更は**別 issue** または本 issue の拡張として切り出す。

## 問題解決

### 問題だったこと

1. **macOS I420**: Rust が `uv_data` に使うバイト長が、C の `video_c.m` で確保している `uvSize = strideUV * chromaHeight * 2` と式レベルで一致しているか未整理だった。特に奇数 `height` では「`stride_uv * height` で足りる」という誤りがありうる。
2. **V4L2 NV12**: `bytesperline` を stride に使わず `session->width` を渡しているため、Rust の下限防御と実バッファ境界が必ずしも一致しないリスクを、コメントで明示する必要があった（本 issue のスコープは「数学的に C と Rust を一致させた」とは書かない）。

### どう解決したか

1. **macOS**: `src/video_c.m` の `uvSize` 算出直後に、Rust `i420_plane_sizes` の UV 式と一致することを日本語コメントで記載した。`src/capture.rs` の `i420_plane_sizes` に、C の `chromaHeight` と同じ `((height + 1) / 2)` を使った **`stride_uv * chroma_h * 2`（`checked_mul`）** と doc を書いた。Y は `stride * height` のまま。
2. **V4L2**: `src/video_v4l2.c` の NV12 コールバック付近に、`bytesperline` と `session->width` の乖離しうる旨の日本語コメントを追加した（C の stride 渡しは変更していない）。

### issue 本文・関連 issue との差異

- **0008** の issue 草案では I420 の UV を `stride_uv * height` としていたが、上記の macOS 突き合わせの結果、**0008 の実装は 0004 と同じ式に統一した**（詳細は **0008** の「問題解決」を参照）。

## 完了条件（チェックリスト）

- [x] macOS I420 について、C の `uvSize` と Rust の UV バイト長が式レベルで突き合わせられ、コメントまたは文書に残っている。
- [x] V4L2 を触る場合は、**bytesperline 未使用の限界**をコメントに書くか、コード変更まで含めるかを明示したうえで対応する（無理に「一致した」と書かない）。

## 完了条件の検証

2026-04-02 にソースを確認した。

- macOS: `video_c.m` に `uvSize` と Rust `i420_plane_sizes` の UV 式が一致する旨の日本語コメント（約 78〜82 行）。`capture.rs` の `i420_plane_sizes` に C と同じ `chroma_h`・`checked_mul` による UV 長（約 123〜138 行）。
- V4L2: `video_v4l2.c` の NV12 コールバックに `bytesperline` と `session->width` の乖離しうる旨の日本語コメント（約 556〜557 行）。C の stride 渡しは未変更のまま限界を明示。

## 変更履歴について

- 現状リポジトリに **`CHANGES.md` は無い**。変更履歴を別途導入する運用になった場合は `AGENTS.md` に従う。**本 issue の完了条件に `CHANGES.md` の追記は含めない**（導入済みになった時点でルールに合わせる）。

## 参考

- `src/video_c.h` の `FrameCallback` 引数説明（26 行付近）。
- `src/video_v4l2.c`: NV12 コールバック（554 行付近）。
