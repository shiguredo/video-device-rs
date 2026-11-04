# CI/CD のハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-ci-cd-hardening
- Polished: 2026-11-04

## 目的

CI/CD ワークフローに 3 つの不備がある。(1) `mjpeg` feature のコードパスが一切テストされていない、(2) `shiguredo/github-actions` の共有アクションが `@main` 参照のままでコミットハッシュに固定されていない、(3) 任意のタグ push でリリース + publish が発火する。これらを修正する。

MSRV 検証は `rust-toolchain.toml` の `channel = "1.95"` と CI の `rustup show` により、Rust をビルドする CI ジョブで既に担保されているため、本 issue では追加ジョブを設けない。

## 優先度根拠

- Medium。(1) は `enable_mjpeg` cfg のコードがコンパイルもテストもされていないカバレッジの穴、(2) はサプライチェーン攻撃のリスク、(3) は任意タグによるリリース + publish の発火リスク
- いずれもランタイムのバグではないが、品質保証・セキュリティの観点で早期に対応すべき
- `/review-code` の重要指摘として確認

## 現状

### (1) mjpeg feature 未テスト（`.github/workflows/ci.yml` の `ci` ジョブの matrix）

CI matrix に `--features mjpeg` のジョブがない。`src/frame_math.rs` の `mjpeg_payload_bytes` 関数（`#[cfg(enable_mjpeg)]`）と、`src/capture_ffi.rs` の `PixelFormat::Mjpeg` 分岐にある `#[cfg(enable_mjpeg)]` 側のペイロード変換経路が、コンパイルもテストもされていない。

### (2) 共有アクションが @main（`.github/workflows/ci.yml` の `rust-cache` と `slack-notify`、`.github/workflows/release.yml` の `slack-notify`）

```yaml
uses: shiguredo/github-actions/.github/actions/rust-cache@main
uses: shiguredo/github-actions/.github/actions/slack-notify@main
```

同一ファイル内で `actions/checkout` はコミットハッシュでピン留めしている一方、`shiguredo/github-actions` は `@main` のままである。

`shiguredo/github-actions` にはリリースおよびタグが存在しない（2026-11-04 時点で `git ls-remote --tags` と GitHub API の `releases` / `tags` が空であることを確認済み）。同日時点の `main` の HEAD は `27c6a25432cf731e39887c8cd77b10f5465cb24c` である。

### (3) タグバリデーションなし（`.github/workflows/release.yml` の `on.push.tags`）

```yaml
on:
  push:
    tags:
      - "*"
```

`test-tag` のようなバージョン形式でないタグを push しても GitHub Release が作成され、`cargo publish` が試行される。

### (4) MSRV 検証（対応済み・追加対応なし）

`Cargo.toml` の `rust-version` は `1.95`、`rust-toolchain.toml` の `channel` も `1.95` で一致している。`.github/workflows/ci.yml` は `rustup show` で `rust-toolchain.toml` のツールチェーンを導入し、Rust をビルドするジョブは MSRV でビルドする。したがって MSRV 検証専用のジョブは不要であり、本 issue の完了条件には含めない。

## 設計方針

### (1) の修正

`ci` ジョブの matrix に mjpeg ジョブを追加する:

```yaml
- name: Ubuntu (v4l2,mjpeg)
  os: ubuntu-24.04
  cargo-flags: "--features mjpeg"
```

`mjpeg` feature は `v4l2` を含むため、V4L2 のビルドと mjpeg 関連のユニットテストが実行される。実デバイスが必要な MJPEG キャプチャ経路は `device-test` の対象外である。

### (2) の修正

`shiguredo/github-actions` のアクションをコミットハッシュでピン留めする。`shiguredo/github-actions` はリリースタグを持たないため、`main` の最新コミットハッシュを調査して固定し、`# main` のコメントを付ける。これにより `update-actions` はブランチ HEAD として追従できる。

`update-actions` スキルはブランチ参照を「対象外」とするが、本 issue ではサプライチェーン対策のため明示的にハッシュ固定にする。

### (3) の修正

タグパターンをバージョン形式に制限する:

```yaml
on:
  push:
    tags:
      - "[0-9]+.[0-9]+.[0-9]+"
      - "[0-9]+.[0-9]+.[0-9]+-canary.[0-9]+"
```

GitHub Actions のタグフィルタは glob パターンであり、`[0-9]+` は 1 桁以上の数字、`.` はリテラルのドットとして解釈される。この形式は `2026.4.0` や `2026.4.0-canary.0` に一致し、`test-tag` には一致しない。

### (4) の対応

追加対応なし。`Cargo.toml` の `rust-version` と `rust-toolchain.toml` の `channel` を一致させ、CI の `rustup show` で MSRV を導入する現在の構成を維持する。

## 完了条件

- (1) CI matrix に mjpeg ジョブを追加する
- (2) `shiguredo/github-actions` のアクションをコミットハッシュでピン留めする
- (3) `release.yml` のタグパターンをバージョン形式に制限する
- 既存の CI ジョブが引き続き成功する（matrix への追加と action のピン留め以外の変更をしない）
- `CHANGES.md` の `## develop` の `### misc` にエントリを追加する

## 解決方法

{完了時に記入}
