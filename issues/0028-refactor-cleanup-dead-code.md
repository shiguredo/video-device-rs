# コードベース内の死にコード・冗長な記述を整理する

- Priority: Low
- Created: 2026-06-15
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/refactor-cleanup-dead-code
- Polished: 2026-06-15

## 目的

`/review-code` の観点 6「削除候補検出」で報告された死にコード・冗長記述のうち、観察可能な挙動を変えない範囲のものをまとめて整理する。読み手の認知負荷を下げて AGENTS.md「Don't live with broken windows」「Premature Optimization is the Root of All Evil」を守る。

## 優先度根拠

- Low。個々の項目は単体では実害が小さいが、放置すると broken window が増える
- 公開 API およびランタイム挙動を変えない範囲に絞るため、リスクは限定的
- `/review-code` の削除候補レポートに基づく具体的な変更リスト

## 現状

`/review-code` で確認された削除候補のうち、本 issue で対応する項目を A 〜 F とする。`MJPEG_MAX_PAYLOAD_BYTES` のコメント短縮は本 issue から除外する (理由は「採用しない案」を参照)。

### A. `src/capture_mf.rs:471` 未使用の `max_length` 変数

```rust
let mut max_length: u32 = 0;
// ...
if buffer.Lock(&mut data_ptr, Some(&mut max_length), Some(&mut current_length)).is_err() { ... }
```

`max_length` は `Lock` の第 2 引数として渡すだけで、それ以降の処理で読み出していない。`Lock` の第 2 引数は `Option<*mut u32>` で `None` を渡せる。

修正方針: `max_length` のローカル束縛を削除し、`Lock` の第 2 引数に `None` を渡す。

### B. `src/capture_mf.rs:571-576` `PixelFormat::Mjpeg` 分岐の過剰コメント短縮

```rust
PixelFormat::Mjpeg => {
    // 本分岐に到達するのはバグ。pixel_format_to_guid(Mjpeg) = None により
    // get_configured_format で UnsupportedPixelFormat(Mjpeg) が先に返るため。
    // 万一到達しても安静にドロップせず、明示的に異常系として扱う。
    return;
}
```

`pixel_format_to_guid(Mjpeg) = None` (`src/types.rs:386`) により `create_source_reader` (`src/capture_mf.rs:302-306`) で `Error::UnsupportedPixelFormat(Mjpeg)` が返り、`MfCaptureImpl::new` の構築が失敗するため `process_sample` には到達しない。コメントが冗長。

修正方針: `return;` の挙動は変えず、コメントを 1 行に短縮する。

```rust
PixelFormat::Mjpeg => {
    // pixel_format_to_guid(Mjpeg) = None で create_source_reader が UnsupportedPixelFormat を返すため到達しない
    return;
}
```

注: `unreachable!()` への置き換えは行わない。理由は型システムで「到達しない」ことが保証されているわけではなく、将来 MF 側で MJPEG 対応が増えた場合に隠れた panic 経路になりうるため。本 issue は「観察可能な挙動を変えない」スコープに留める。

### C. `src/types.rs:88-115` `PixelBuffer::from_retained_ptr` の Linux 経路に契約検知を追加

```rust
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
pub(crate) unsafe fn from_retained_ptr(ptr: *mut c_void) -> Option<Self> {
    if ptr.is_null() {
        return None;
    }
    #[cfg(enable_avf)]
    {
        Some(Self { ptr })
    }
    #[cfg(not(enable_avf))]
    {
        // Linux では Drop で CFRelease しないため、非 NULL を保持するとリークしうる。契約上 NULL のみ。
        None
    }
}
```

`#[cfg(not(enable_avf))]` 分岐は常に `None` を返し、入力 `ptr` を捨てる。Linux で C 側がうっかり非 NULL を渡すと黙ってリークする可能性がある。

修正方針 (シグネチャは変えない):

1. `from_retained_ptr` のシグネチャと `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ガードは現状維持する (`capture_ffi.rs:260` から呼ばれるため `#[cfg(enable_avf)]` 限定化はできない)
2. `#[cfg(not(enable_avf))]` 分岐に `debug_assert!(ptr.is_null(), "non-null pixel_buffer pointer is not supported outside enable_avf");` を追加し、デバッグビルドで C 側の契約違反を検知できるようにする
3. リリースビルドでは従来通り `None` を返す (実害を最小化)
4. doc コメント (`src/types.rs:88` 周辺) の「macOS 以外では C からは常に NULL が渡る想定であり、非 NULL の `from_retained_ptr` 取り込みは行わない（サポート外）」を維持する

`capture_ffi.rs:260` 側は変更しない (現状の `unsafe { PixelBuffer::from_retained_ptr(pixel_buffer) }` 呼び出しを継続する)。

### D. `build.rs:22, 24` `target_os` の重複フェッチ

```rust
let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
let mut enable_default_count = 0;
match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
    // ...
}
```

L22 で `target_os` を取得しているのに L24 で再度同じ env を取得している。L62 / L72 / L103 では `target_os.as_str()` が使われており、L24 だけ重複フェッチ。

修正方針: L24 の `match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str()` を `match target_os.as_str()` に揃える。

### E. `build.rs:165` bindgen `allowlist_var("VIDEO_PIXEL_FORMAT_.*")` 削除

```rust
.allowlist_var("VIDEO_PIXEL_FORMAT_.*")
```

`grep "ffi::VIDEO_PIXEL_FORMAT" src/` でヒットせず、Rust 側から bindgen 生成定数を参照していない。Rust 側は `src/types.rs:11-14` で独自の `pub(crate) const VIDEO_PIXEL_FORMAT_*` を定義している。bindgen の `allowlist_var` 行は不要。

修正方針: `allowlist_var("VIDEO_PIXEL_FORMAT_.*")` 行を削除する。生成バインディングからは `VIDEO_PIXEL_FORMAT_*` が外れるが、`ffi::` 経由で参照していないので影響なし。

注: 「Rust 側と C 側の定数定義を一元化する」リファクタリングはスコープ外。別 issue (refactor / change カテゴリ) で扱う。

### F. `src/types.rs:157-158` 過剰コメントの短縮

```rust
// Core Foundation の参照カウントはスレッドセーフで、保持しているのは不透明ポインタのみ。
unsafe impl Send for PixelBuffer {}
```

「保持しているのは不透明ポインタのみ」は `ptr: *mut c_void` (`src/types.rs:91`) を見れば自明。1 行に短縮する。

```rust
// Core Foundation の参照カウントはスレッドセーフ。
unsafe impl Send for PixelBuffer {}
```

## 採用しない案 (本 issue から除外する項目)

- **`src/video_v4l2.c:17-27` の `MJPEG_MAX_PAYLOAD_BYTES` 11 行コメント短縮**: closed/0020 (V4L2 MJPEG パススルー対応) で意図的に追加された詳細な根拠コメントで、`256 MiB` を選んだ理由 (8K HDR MJPEG 90 MiB 超への対応、異常ドライバ防御、INT32_MAX 等) を将来このマジックナンバーを見直す保守者向けに残している。AGENTS.md「コメントはしっかり入れること」と直接矛盾するため短縮しない。`shiguredo-issues` 規約により「ソースコード本体に issue 番号を書かない」ため、closed/0020 へのリンクで代替するのも不可。よって現状コメントを維持する
- **`PixelBuffer` 全体の `#[cfg(enable_avf)]` 化**: `VideoFrame.pixel_buffer` の型が変わる破壊的変更を伴うためスコープ外
- **`PixelFormat::Unknown(u32)` の公開 API からの除去**: 破壊的変更を伴うためスコープ外
- **`types.rs` の Windows 専用ユーティリティ (`CoInitGuard` 等) を別モジュールに移動**: ファイル移動を伴う大きな構造変更で、本 issue (死にコード・冗長記述の整理) のスコープを超えるため別 issue で扱う
- **`examples/README.md` と `README.md` の重複削除**: ドキュメント整理 issue で扱う
- **`docs/LINUX.md` と `README.md` の PipeWire コマンド齟齬**: ドキュメント整理 issue で扱う
- **`CHANGES.md ### misc` の不要記述削除**: `shiguredo-changelog` 規約の運用判断であり別途検討する

## 影響範囲

- `src/capture_mf.rs`: A (max_length 削除), B (コメント短縮)
- `src/types.rs`: C (`debug_assert!` 追加), F (コメント短縮)
- `build.rs`: D (target_os 重複フェッチ削除), E (`allowlist_var` 削除)
- `src/video_v4l2.c`, `src/capture_ffi.rs`, `src/video.h`, `src/device_*`: 変更なし
- 公開 API: 変更なし (`PixelFormat`, `VideoFrame`, `VideoCapture`, `VideoDevice` のシグネチャすべて不変)
- ランタイム挙動: 変更なし (`debug_assert!` はデバッグビルドのみ、リリースビルドの挙動は不変)

## 実装手順

A 〜 F の 6 項目を **1 つのコミットにまとめて** 修正する。`shiguredo-issues` の「1 issue 1 ブランチ 1 PR」「1 PR 1 squash merge」運用に合わせる。

## 完了条件

- A 〜 F の 6 項目を 1 コミットで実装する
- 対象 OS 上で次のコマンドが通る。OS 別に有効な feature 組み合わせのみ検証する (`avf` は macOS 専用、`mf` は Windows 専用、`v4l2` / `pipewire` は Linux 専用のため、全 feature を 1 OS で同時に有効化することはできない)
  - Linux: `cargo build --workspace` (default = `v4l2 + avf + mf` だが `avf` / `mf` は build.rs で OS に応じて gate される)、`cargo build --workspace --no-default-features --features v4l2`、`cargo build --workspace --no-default-features --features pipewire`、`cargo build --workspace --no-default-features --features default-pipewire` (`.github/workflows/ci.yml` の `Ubuntu (pipewire)` ジョブと同形式)、`cargo build --workspace --features v4l2,pipewire` (`Ubuntu (v4l2,pipewire)` ジョブと同形式)、`cargo build --workspace --no-default-features --features v4l2,mjpeg`
  - macOS: `cargo build --workspace`、`cargo build --workspace --no-default-features --features avf`
  - Windows: `cargo build --workspace`、`cargo build --workspace --no-default-features --features mf`
- 上記すべての組み合わせで `cargo test --workspace` および `cargo clippy --workspace --all-targets -- -D warnings` が通る
- 残りのプラットフォーム feature 組み合わせは CI で検証する (`.github/workflows/ci.yml` の `Ubuntu (v4l2)` / `Ubuntu (pipewire)` / `Ubuntu (v4l2,pipewire)` / `macOS` / `Windows` ジョブ)
- 既存テストのカバレッジが減らない (削除候補は死にコードのため、もとよりテストでカバーされていない)
- `CHANGES.md` の `## develop` の `### misc` セクションに以下のエントリを追記する。本 issue は公開 API およびランタイム挙動を変えない内部リファクタのため `[UPDATE]` (後方互換あり) を使い、`### misc` に置く

  ```markdown
  - [UPDATE] 内部の死にコード・冗長コメントを整理する
    - @<github-id>
  ```

## 解決方法

{完了時に記入}
