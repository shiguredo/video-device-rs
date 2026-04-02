# フレームコールバックの契約を文書化する

Created: 2026-04-02  
Model: Composer 2 Fast

## 目的

パニック（FFI 経由）・利用者誤用に起因する解放後使用（セグフォ／未定義動作）のリスクを、**文書で明示**する。

## 背景

- コールバックの実体は `src/capture.rs` の `extern "C" fn frame_callback` から `CaptureContext` 内の `Fn(VideoFrame<'_>)` を呼ぶ経路。
- ユーザコールバックが**パニック**すると、`extern "C"` の外にアンワインドが及ぶ可能性があり、ABI 上よくない。
- macOS I420（`src/video_c.m` 約 69〜95 行付近）では、連結 UV 用の `uvBuffer` を `calloc` し、**`self.callback(...)` の直後に `free(uvBuffer)`** している。コールバックが返るまで有効なのは **`VideoFrame` のスライス参照が指すメモリ**であり、**非同期にスライスだけを渡して後から読むと解放後使用**になりうる。

## 修正の入れどころ（すべて満たすこと）

1. **`src/video_c.h`**  
   `FrameCallback` の typedef 直上のコメントブロックに、英語で次を短く書く:
   - コールバックはパニックしてはならない（または同等の注意）。
   - `uv_data` や `data` が指すバッファは、コールバックが返るまで有効。非同期にスライスを保持しないこと。必要ならコピーまたは上位 API の `to_owned()` を使うこと。
2. **`src/lib.rs` のクレートドキュメント**（先頭 `//!`）  
   `VideoCapture` を使う利用者向けに、上記と同趣旨を**日本語で 2〜4 文**（パニック禁止・同期処理／`VideoFrameOwned` へのコピー）。
3. **`types.rs` の `VideoFrame` と `VideoFrameOwned`（契約を分ける）**
   - **`VideoFrame`**: 借用のフレーム。**スライスが指すメモリの寿命はコールバック呼び出し中に限る**旨を 1 文（日本語）。
   - **`VideoFrameOwned`**: **所有データ**。コールバック終了後も保持してよい。長期保持や別スレッドへ渡す用途はこちらを使う、と**逆の説明**を 1 文（日本語）。`VideoFrame` と同じ「寿命はコールバック中のみ」とは書かない。

## 完了条件（チェックリスト）

- [x] `video_c.h` に英語の注意書きがある。
- [x] `lib.rs` に利用者向けの日本語の注意がある。
- [x] `VideoFrame` に借用の寿命制約がある。
- [x] `VideoFrameOwned` に所有型としての説明があり、上記と矛盾しない。
- [x] ログ・エラーメッセージは英語のまま（新規は英語）。

## 完了条件の検証

2026-04-02 にソースを確認した。

- `video_c.h`: `FrameCallback` 直上に英語でパニック禁止・ポインタ寿命・`to_owned()`（約 33〜35 行）。
- `lib.rs`: クレート doc に日本語でコールバック契約（約 6〜10 行）。
- `types.rs`: `VideoFrame` は寿命がコールバック中のみ、`VideoFrameOwned` はコールバック後も保持可と対比（約 195〜238 行）。
- エラー・ログ文言: `error.rs` 等の利用者向けメッセージは英語のまま（本件で追加した doc コメントの日本語は完了条件の対象外）。

## 参考コード位置

- `src/capture.rs`: `frame_callback` 末尾の `(context.callback)(frame)`。
- `src/video_c.m`: I420 分岐の `free(uvBuffer)`（コールバックの直後）。
- `src/types.rs`: `VideoFrame`（約 184 行付近）、`VideoFrameOwned`（約 214 行付近）。
