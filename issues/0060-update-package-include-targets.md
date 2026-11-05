# crate の同梱物とパッケージ時警告を整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-package-include-targets
- Polished: 2026-11-05

## 目的

`Cargo.toml` の `include` は `/LICENSE` / `/README.md` / `/build.rs` / `/src/**` のみで、`examples/` / `tests/` / `docs/` が公開パッケージに含まれない。

このため次の問題がある。

- `cargo package` が 6 件の警告を出す（examples 3 件、tests 3 件が「公開パッケージに含まれないため無視する」）
- README の `docs/LINUX.md` への相対リンクが docs.rs の crate ページで 404 になる。crates.io の README 表示では相対リンクが GitHub の絶対 URL に書き換えられるため crates.io では壊れない
- README のサンプル節が `cargo run --example ...` を案内しているが、パッケージには examples が含まれない
- `Cargo.toml` の `[[example]]` セクション（`name = "camera_preview"`）は自動検出と同一設定で no-op であり、警告を消す効果も無い

## 優先度根拠

- Low。公開物の品質と利用者体験の問題であり、ランタイムの不具合ではない
- `/review-code` の指摘として確認済み

## 現状

- `cargo package --list` の出力は 25 ファイルで、`examples/` / `tests/` / `docs/` を含まない（`Cargo.lock` と `.cargo_vcs_info.json` は Cargo が自動的に含む）
- `cargo package --allow-dirty` は成功し検証ビルドも通るが、examples 3 件（`camera_preview` / `device_info` / `device_list`）と tests 3 件（`test_capture` / `test_pipewire` / `test_types`）の警告が 6 件出る
- README の `docs/LINUX.md` への相対リンクは 2 箇所ある。crates.io の README 表示では
  `https://github.com/shiguredo/video-device-rs/blob/HEAD/docs/LINUX.md` に書き換えられるため壊れないが、
  docs.rs の crate ページ (`https://docs.rs/crate/shiguredo_video_device/latest`) では
  `https://docs.rs/crate/shiguredo_video_device/latest/docs/LINUX.md` となり 404 になる
- README のサンプル節は `cargo run --example device_list` / `cargo run --example device_info` / `cargo run --example camera_preview` を案内しているが、パッケージに examples は含まれない
- `[[example]]` は `name = "camera_preview"` のみで、`required-features` 等を持たない。自動検出と同一であり、`cargo package` の警告は `[[example]]` を残したままでも出る

## 設計方針

- `Cargo.toml` の `include` に `/examples/**` / `/tests/**` / `/docs/**` を追加する。これで `cargo package` の警告 6 件を 0 件にする
- `[[example]]` セクション（`name = "camera_preview"`）を削除する。自動検出で `camera_preview` は引き続きビルドできる
- README の `docs/LINUX.md` への相対リンク 2 箇所を `https://github.com/shiguredo/video-device-rs/blob/develop/docs/LINUX.md` に変更する。docs/ を同梱しても docs.rs の crate ページの相対リンクは直らないため、include の追加とは独立して修正する
- README と `examples/README.md` のサンプル説明の重複整理は
  `issues/0064-doc-docs-comments-consistency.md` で扱う。
  本 issue では examples を同梱することで、README と `examples/README.md` が案内する
  `cargo run --example ...` とパッケージ内容を一致させる

## 完了条件

- `cargo package --allow-dirty` の警告が 0 件である
- `cargo package --list` に `examples/` / `tests/` / `docs/` が含まれる
- README の `docs/LINUX.md` への 2 箇所のリンクが、docs.rs の crate ページでも解決する GitHub の絶対 URL になっている
- README / `examples/README.md` / `docs/LINUX.md` が案内する `cargo run --example ...` の対象がパッケージに含まれている
- `cargo package` の検証ビルドが通る

## 解決方法

{完了時に記入}
