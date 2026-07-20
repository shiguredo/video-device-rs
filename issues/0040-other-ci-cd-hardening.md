# CI/CD のハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-ci-cd-hardening
- Polished: {YYYY-MM-DD}

## 目的

CI/CD ワークフローに 4 つの不備がある。(1) `mjpeg` feature のコードパスが一切テストされていない、(2) `shiguredo/github-actions` の共有アクションが `@main` でピン留めされていない、(3) 任意のタグ push でリリース + publish が発火する、(4) MSRV の検証ジョブがない。これらを修正する。

## 優先度根拠

- Medium。(1) は `enable_mjpeg` cfg のコードがコンパイルもテストもされていないカバレッジの穴、(2) はサプライチェーン攻撃のリスク、(3) は誤ったバージョンの publish リスク、(4) は MSRV 破壊の検出不能
- いずれもランタイムのバグではないが、品質保証・セキュリティの観点で早期に対応すべき
- `/review-code` の重要指摘として確認

## 現状

### (1) mjpeg feature 未テスト（.github/workflows/ci.yml:19-38）

CI matrix に `--features mjpeg` のジョブがない。`src/frame_math.rs:77-83` の `#[cfg(enable_mjpeg)]` コード、`src/capture_ffi.rs:345-368` の MJPEG 経路がコンパイルもテストもされていない。

### (2) 共有アクションが @main（.github/workflows/ci.yml:45,174, .github/workflows/release.yml:64）

```yaml
uses: shiguredo/github-actions/.github/actions/rust-cache@main
uses: shiguredo/github-actions/.github/actions/slack-notify@main
```

同一ファイル内で `actions/checkout` はコミットハッシュでピン留め（ci.yml:43）している一方、`shiguredo/github-actions` は `@main`。

### (3) タグバリデーションなし（.github/workflows/release.yml:4-6）

```yaml
on:
  push:
    tags:
      - "*"
```

`test-tag` のようなバージョン形式でないタグを push しても GitHub Release が作成され、`cargo publish` が試行される。

### (4) MSRV 検証ジョブなし

`Cargo.toml` に `rust-version = "1.88"` を宣言しているが、CI では `rustup update stable` のみ。MSRV でビルドできることを保証するジョブがない。

## 設計方針

### (1) の修正

CI matrix に mjpeg ジョブを追加する:

```yaml
- name: Ubuntu (v4l2,mjpeg)
  os: ubuntu-24.04
  cargo-flags: "--features mjpeg"
```

### (2) の修正

`shiguredo/github-actions` のアクションをコミットハッシュでピン留めする。最新リリースのコミットハッシュを調査して固定する。

### (3) の修正

タグパターンをバージョン形式に制限する:

```yaml
on:
  push:
    tags:
      - "[0-9]+.[0-9]+.[0-9]+"
      - "[0-9]+.[0-9]+.[0-9]+-canary.[0-9]+"
```

### (4) の修正

MSRV 検証ジョブを追加する:

```yaml
- name: MSRV
  runs-on: ubuntu-24.04
  steps:
    - uses: actions/checkout@...
    - run: rustup override set 1.88
    - run: cargo check --workspace
```

## 完了条件

- (1) CI matrix に mjpeg ジョブを追加する
- (2) `shiguredo/github-actions` のアクションをコミットハッシュでピン留めする
- (3) release.yml のタグパターンをバージョン形式に制限する
- (4) MSRV 検証ジョブを追加する
- 既存の CI ジョブの挙動が変わらない
- `CHANGES.md` の `## develop` の `### misc` にエントリを追加する

## 解決方法

{完了時に記入}
