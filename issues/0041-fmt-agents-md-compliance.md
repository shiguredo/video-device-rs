# AGENTS.md 規約違反の修正

- Priority: Low
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/refactor-agents-md-compliance
- Polished: {YYYY-MM-DD}

## 目的

ソースコード内に AGENTS.md の規約に違反する記述が 4 箇所ある。(1) `tests/test_capture.rs` のログメッセージが英語（規約: テストのログメッセージは日本語）、(2) `src/frame_math.rs:169` のテストコメントが英語（規約: コメントは日本語）、(3) `src/video_pipewire.c:218-220` に issue への言及が残っている（規約: ソースコードに issue 参照を書かない）、(4) `examples/` のログメッセージが日本語・英語混在（規約: ログメッセージは英語）。これらを修正する。

## 優先度根拠

- Low。機能的な問題はないが、AGENTS.md 規約違反であり「Don't live with broken windows」に反する
- 同一リポジトリ内で `test_pipewire.rs` は日本語、`test_capture.rs` は英語と不統一
- `/review-code` の重要指摘（規約違反）として確認

## 現状

### (1) test_capture.rs のログメッセージが英語

`tests/test_capture.rs` の全 expect / assert / eprintln メッセージが英語。例:

- :18 `expect("device enumeration failed")`
- :20 `"no video device found"`
- :64 `panic!("timeout waiting for frame {}/{}", ...)`
- :80 `eprintln!("test_capture_frames: dropped frame ...")`

AGENTS.md:13「テストのログメッセージは全て日本語にすること」。同一リポジトリの `tests/test_pipewire.rs` は全メッセージ日本語（例: :16 `"PipeWire デーモンが起動していて、少なくとも 1 台のカメラが接続されていること"`）。

### (2) frame_math.rs:169 のテストコメントが英語

```rust
// YUY2: 2 bytes per pixel, so stride = width * 2
```

AGENTS.md:11「コメントは全て日本語にすること」。同ファイルの他テストコメント（:104, :111 等）は日本語または数式表記。

### (3) video_pipewire.c:218-220 の issue 言及

```c
// fps は本 issue では未取得のため仮値を埋める
// - 0.0 は CI の Device Test ジョブが all(.max_fps > 0) を要求しているため使えない
// - 正確な fps 抽出 (choice 形式の展開) は別 issue で扱う
```

`shiguredo-issues` 規約「issue 番号・issue への言及をソースコードに持ち込まないこと」に違反。「本 issue」「別 issue」はどの issue を指しているか特定できない。

### (4) examples のログメッセージが不統一

- `examples/device_list.rs:9` — `eprintln!("デバイスの列挙に失敗しました: {e}")`（日本語）
- `examples/device_info.rs:11` — 同上（日本語）
- `examples/camera_preview.rs:88,96,103,110,114,125,134` — `eprintln!("エラー: ...")`（日本語）
- `examples/camera_preview.rs:222-224,261,277,319-321` — 英語

AGENTS.md:12「ログメッセージは全て英語にすること」。examples はテストではないため英語が正。

## 設計方針

### (1) の修正

`tests/test_capture.rs` の全 expect / assert / eprintln メッセージを日本語に翻訳する。`test_pipewire.rs` の文体に合わせる。

### (2) の修正

```rust
// YUY2: 1 ピクセル 2 バイトのため stride = width * 2
```

### (3) の修正

issue への言及を削除し、理由そのもの（CI の制約）だけを残す:

```c
// fps は未取得のため仮値を埋める
// - 0.0 は CI の Device Test ジョブが all(.max_fps > 0) を要求しているため使えない
// - 正確な fps 抽出 (choice 形式の展開) は別途対応
```

### (4) の修正

`examples/device_list.rs`、`examples/device_info.rs`、`examples/camera_preview.rs` の日本語ログメッセージを英語に統一する。

## 完了条件

- (1) `tests/test_capture.rs` の全ログメッセージを日本語にする
- (2) `src/frame_math.rs:169` のコメントを日本語にする
- (3) `src/video_pipewire.c:218-220` の issue 言及を削除する
- (4) `examples/` のログメッセージを英語に統一する
- `cargo build --workspace` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する

## 解決方法

{完了時に記入}
