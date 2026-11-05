# ビルド・動作要件（PipeWire の最低バージョン）を修正する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-build-runtime-requirements
- Polished: 2026-11-05

## 目的

README.md / docs/LINUX.md のビルド・動作要件のうち、PipeWire の最低バージョンだけが未記載である。libclang / 最低 macOS バージョン / ランタイム依存は追記済みで、残りを完了させる。

## 優先度根拠

- Medium。Ubuntu 22.04 の PipeWire は 0.3.48 であり、0.3.49 で導入された `pw_init` / `pw_deinit` の参照カウントが無い。
  `src/video_pipewire.c` は列挙とキャプチャで `pw_init` / `pw_deinit` を対で呼ぶため、0.3.48 以前では列挙とキャプチャの並行利用時に
  `pw_deinit` がグローバル状態を早期に解放し、初期化失敗やクラッシュの原因になりうる
- 利用者が最初に参照するドキュメントの記載漏れであり、早期修正が望ましい
- `/review-code` の重要指摘として確認済み

## 現状

- README.md の「## 動作要件」は Rust 1.95 以上と macOS 14.0 以上を明記している。macOS 14.0 以上は `AVCaptureDeviceTypeExternal` を利用するためである
- README.md の「### Ubuntu (Linux)」と docs/LINUX.md の「### ビルド依存」は `libclang-dev` を明記している
- README.md と docs/LINUX.md の PipeWire 節は `libclang-dev` / `libpipewire-0.3-dev` / `pkg-config` を明記し、実行時に `libpipewire-0.3` の共有ライブラリと PipeWire デーモンが必要であることを明記している
- `.github/workflows/ci.yml` と `.github/workflows/release.yml` は Linux で `libclang-dev` を導入している
- docs/LINUX.md と README.md の PipeWire 節には PipeWire の最低バージョンが無い
- `src/video_pipewire.c` の `video_pipewire_enumerate_devices` は `pw_init` と `pw_deinit` を対で呼び、`video_pipewire_session_create` は `pw_init` を呼び、`video_pipewire_session_destroy` が `pw_deinit` を呼ぶ
- PipeWire 0.3.49 の `src/pipewire/pipewire.c` では `pw_init` が `init_count` を増やし、`pw_deinit` が 0 になるまで解放しない。PipeWire 0.3.48 の同ファイルでは `pw_init` が `initialized` を立てるだけで、`pw_deinit` は参照カウントを見ずにグローバル状態を解放する
  - <https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/0.3.48/src/pipewire/pipewire.c>
  - <https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/0.3.49/src/pipewire/pipewire.c>
- `build.rs` は `MACOSX_DEPLOYMENT_TARGET` / `-mmacosx-version-min` を明示していない。`cc` クレートが `MACOSX_DEPLOYMENT_TARGET` または SDK の deployment target を使用する

## 設計方針

- docs/LINUX.md の PipeWire 節に「PipeWire 0.3.49 以上が必要」と明記する。0.3.49 で `pw_init` / `pw_deinit` の参照カウントが導入され、本ライブラリの `pw_init` / `pw_deinit` の対称呼び出しが並行利用時にグローバル状態を早期に解放されるのを防げるためである
- README.md の PipeWire 節にも同じ最低バージョンを 1 行追記する。README.md は利用者が最初に参照するため
- 最低 macOS バージョン（14.0 以上）は README.md に明記済み。
  `@available` / `AVCaptureDeviceTypeExternalUnknown` へのフォールバックは行わず、macOS 14.0 以上を前提とする。
  `issues/0049-bug-avf-device-selection.md` の「README のとおり macOS 14.0 以上を前提とするため、`@available` は不要」と整合させる
- `build.rs` は変更しない。README.md で macOS 14.0 以上を要求するため、deployment target の明示は必須としない
- libclang とランタイム依存の記述は追記済みのため、追加の変更は不要

## 完了条件

- docs/LINUX.md の PipeWire 節に PipeWire 0.3.49 以上が必要であることが明記されている
- README.md の PipeWire 節にも PipeWire 0.3.49 以上が必要であることが明記されている
- README.md と docs/LINUX.md のビルド手順が CI の `libclang-dev` / `libpipewire-0.3-dev` / `pkg-config` の導入と矛盾しない
- ランタイム依存の記述が `pipewire` feature の有無と整合している
- 最低 macOS バージョン 14.0 以上が README.md に明記されている
- README.md と docs/LINUX.md の手順だけで Ubuntu / macOS / Windows のビルドが成功する

## 解決方法

{完了時に記入}
