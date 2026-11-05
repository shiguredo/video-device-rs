# ビルド・動作要件（libclang / 最低 macOS / 最低 PipeWire / ランタイム依存）を修正する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-build-runtime-requirements
- Polished: {YYYY-MM-DD}

## 目的

README / docs/LINUX.md のビルド・動作要件が実装と一致していない。

1. Linux の V4L2 バックエンドは `build.rs` が bindgen を実行するため libclang が必要だが、README の「Ubuntu (Linux) / V4L2」と docs/LINUX.md の V4L2 節は「追加の依存パッケージは不要」としている。PipeWire 節も `libpipewire-0.3-dev` しか挙げていない。CI とリリースワークフローは libclang-dev を導入している
2. `src/video_avf.m` は `AVCaptureDeviceTypeExternal`（macOS 14.0 以降で追加）を無条件に参照しているが、README の動作要件は Rust のバージョンしか書いておらず、最低 macOS バージョンが無い。deployment target の指定も無いため、macOS 13 以前向けにビルドするとシンボル解決に失敗し、起動できない
3. PipeWire の最低バージョンが書かれていない。`pw_init` / `pw_deinit` の参照カウント挙動は PipeWire 0.3.49 以降を前提とする
4. README の「macOS / Linux はランタイム依存なし」は、`pipewire` feature 有効時に libpipewire-0.3.so を動的リンクするため不正確

## 優先度根拠

- Medium。(1) は記載どおりにビルドすると失敗する、(2) はサポート外 OS で起動時クラッシュする
- 利用者が最初に参照するドキュメントの誤りであり、早期修正が望ましい
- `/review-code` の重要指摘として確認済み

## 現状

- README の「ビルド要件 / Ubuntu (Linux) / V4L2」は「追加の依存パッケージは不要です」としている
- `.github/workflows/ci.yml` と `.github/workflows/release.yml` は Linux で `libclang-dev` を導入している
- `build.rs` は `MACOSX_DEPLOYMENT_TARGET` / `-mmacosx-version-min` を設定していない
- macOS SDK の `AVCaptureDevice.h` は `AVCaptureDeviceTypeExternal` を `API_AVAILABLE(macos(14.0), ...)` と定義している

## 設計方針

- Linux のビルド要件に libclang（Ubuntu では `libclang-dev`）を追加する
- 最低 macOS バージョン（14.0 以上）を README に明記する。併せて `@available` / `AVCaptureDeviceTypeExternalUnknown` へのフォールバックを検討する（実装方針は AVF の issue と整合させる）
- PipeWire の最低バージョンを docs/LINUX.md に明記する
- ランタイム依存の記述を feature ごとに正確化する

## 完了条件

- README と docs/LINUX.md の手順だけで Ubuntu / macOS / Windows のビルドが成功する
- 最低 macOS バージョンと最低 PipeWire バージョンが明記されている
- ランタイム依存の記述が feature の有無と整合している
- 記載内容が CI の実際の手順と一致している

## 解決方法

{完了時に記入}
