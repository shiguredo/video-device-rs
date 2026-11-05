# PipeWire の VideoFormat が常に min_fps 1.0 / max_fps 30.0 の固定値を返す

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-pipewire-fps-fabricated
- Polished: 2026-11-05

## 目的

`src/video_pipewire.c` の `on_node_param` は、デバイスの対応フォーマットを列挙する際に `SPA_FORMAT_VIDEO_framerate` の choice pod を展開せず、`min_fps = 1.0f` / `max_fps = 30.0f` を固定値で埋めている。`VideoFormat` は公開 API であり、`VideoDevice::formats()`（`src/device.rs`）を通じて利用者に渡る。利用者は「このフォーマットは 1〜30fps 対応」と誤認する。

`src/video_pipewire.c` の `video_pipewire_session_start` は、キャプチャ開始時に `SPA_POD_CHOICE_RANGE_Fraction` で 1〜60fps を要求している。デバイスが 60fps を提示する場合、列挙値の `max_fps = 30.0` は要求上限とも一致せず、利用者は 30fps を上限と誤認する。

## 優先度根拠

- Medium。公開 API が実測値ではない値を返す。誤った fps で機器選択・設定を行う利用者に実害がある
- `CHANGES.md` の 2026.4.0 の `[FIX]` エントリに「fps は本修正では仮値 (1.0 / 30.0) で埋まり、choice 形式の正確な fps 抽出は別途対応」と書かれているが、`VideoFormat` の rustdoc には fps の概算 / フォールバックの記載がない
- 0025 で choice 展開は「別 issue」とされたが、その後追跡 issue が起票されていなかった（本 issue が追跡 issue）
- `/review-code` の重要指摘として確認済み

## 現状

```c
// fps は未取得のため仮値を埋める
// - 0.0 は CI の Device Test ジョブが all(.max_fps > 0) を要求しているため使えない
// - 正確な fps 抽出 (choice 形式の展開) は別途対応
p->formats[p->format_count].min_fps = 1.0f;
p->formats[p->format_count].max_fps = 30.0f;
```

`on_node_param` は `spa_format_video_raw_parse` で `SPA_FORMAT_VIDEO_framerate` の解析を試みているが、この API の `SPA_POD_OPT_Fraction` は `SPA_CHOICE_None` 以外の choice を受理しない。`SPA_CHOICE_Range` / `SPA_CHOICE_Step` / `SPA_CHOICE_Enum` の場合は `info.info.raw.framerate` は更新されず、コードはその値を使っていない。

PipeWire の V4L2 プラグインは `SPA_FORMAT_VIDEO_framerate` を `SPA_CHOICE_Enum`（discrete）、`SPA_CHOICE_Range`（continuous）、`SPA_CHOICE_Step`（stepwise）で提示する。PipeWire の libcamera プラグインは `SPA_FORMAT_VIDEO_framerate` を提示しない。

`tests/test_pipewire.rs` は `max_fps > 0.0` を、`.github/workflows/ci.yml` の `device_info` は `all(.max_fps > 0)` を要求している。どちらも `max_fps = 0.0` を許容しないため、現行実装はフォールバックに 1.0 / 30.0 を使っている。ただし、テスト条件は 1.0 / 30.0 という具体的な値を要求しておらず、正の値であればよい。1.0 / 30.0 は choice 未展開の暫定値である。

## 設計方針

- `on_node_param` で `spa_pod_find_prop(param, NULL, SPA_FORMAT_VIDEO_framerate)` により framerate の property を取得し、`spa_pod_get_values` で choice 種別と値列を取得する。`spa_format_video_raw_parse` の `info.info.raw.framerate` は choice を展開しないため使わない
- `SPA_CHOICE_Range` / `SPA_CHOICE_Step` は値列の 2 番目 (min) と 3 番目 (max) を使う。`SPA_CHOICE_Enum` は全値を走査して最小 / 最大を求める。`SPA_CHOICE_None` は単一値を使う。`SPA_CHOICE_Flags` は fraction には使われないため無視する
- `spa_fraction` の `num / denom` を fps として計算する。`denom == 0` の値は無効として除外する。有効な値がない場合、または `max_fps <= 0` もしくは `min_fps > max_fps` の場合は `min_fps = 1.0f` / `max_fps = 30.0f` のフォールバックにする
- `min_fps` は可変レートの場合 0.0 になりうる。`VideoFormat` の rustdoc には、取得できない場合のフォールバックと、可変レートの `min_fps = 0.0` の意味を明記する
- フォールバックは「取得できない場合の概算値」であり、`skills/shiguredo-video-device/SKILL.md` にも明記する。`src/video_pipewire.c` のコードコメントも「CI のため 0.0 を使わない」から「取得できない場合のフォールバック」に更新する
- `tests/test_pipewire.rs` と `.github/workflows/ci.yml` の fps 条件は、`max_fps > 0` を維持したうえで `min_fps <= max_fps` を追加する。フォールバック (1.0 / 30.0) と実測値のどちらでもこの不変条件を満たす
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する
- `issues/0062-doc-public-api-backend-differences.md` の項目 6 は PipeWire を「固定 1.0 / 30.0」と書いているため、0062 の対応時に本 issue の結果に合わせて更新する（本 issue では 0062 を変更しない）

## 変更対象

- `src/video_pipewire.c`: `on_node_param` の framerate choice 展開と、fps フォールバックのコメント更新
- `src/types.rs`: `VideoFormat` の rustdoc に fps のフォールバックと可変レートの `min_fps = 0.0` を明記
- `tests/test_pipewire.rs`: `min_fps <= max_fps` の検証を追加し、既存の `max_fps > 0.0` を維持
- `.github/workflows/ci.yml`: `device_info` の jq 条件（デフォルト / PipeWire の両ステップ）に `min_fps <= max_fps` を追加（`max_fps > 0` は維持）
- `CHANGES.md`: `## develop` に `[FIX]` エントリを追加
- `skills/shiguredo-video-device/SKILL.md`: PipeWire の fps 記述を「デバイスが提示する範囲（取得できない場合は 1.0 / 30.0）」に更新
- `src/video_pipewire.h` / `src/video.h` / `src/device_ffi.rs` / `src/capture_ffi.rs` / `examples/device_info.rs` は変更不要（`VideoFormatEntry` の ABI は変えず、フォールバックは正の値のまま）
- `README.md` / `docs/LINUX.md` は fps の契約を説明していないため変更不要
- `issues/0062-doc-public-api-backend-differences.md` は本 issue では変更しない（0062 の対応時に項目 6 を更新する）

## 完了条件

- `src/video_pipewire.c` の `on_node_param` が `SPA_FORMAT_VIDEO_framerate` の choice を展開し、`SPA_CHOICE_Range` / `SPA_CHOICE_Step` / `SPA_CHOICE_Enum` / `SPA_CHOICE_None` の各形式で `min_fps` / `max_fps` にデバイスが提示する範囲を設定する
- framerate を取得できない場合、または `max_fps <= 0` もしくは `min_fps > max_fps` の場合は `min_fps = 1.0f` / `max_fps = 30.0f` のフォールバックにし、`VideoFormat` の rustdoc と `skills/shiguredo-video-device/SKILL.md` にフォールバックと可変レートの `min_fps = 0.0` を明記する
- `tests/test_pipewire.rs` に `min_fps <= max_fps` の検証を追加し、既存の `max_fps > 0.0` を維持する
- `.github/workflows/ci.yml` の `device_info` の jq 条件（デフォルト / PipeWire の両ステップ）が `max_fps > 0` と `min_fps <= max_fps` を検証し、実装と矛盾しない
- Linux 実機の device test が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する
- `cargo build --workspace --features pipewire` / `cargo clippy --workspace --all-targets --features pipewire -- -D warnings` / `cargo test --workspace --features pipewire` が通る

## スコープ外

- `SPA_FORMAT_VIDEO_maxFramerate` の展開。PipeWire の V4L2 プラグインは `framerate` を提示し、libcamera プラグインは framerate を提示しない。`maxFramerate` を単独で提示するカメラプラグインは現状確認されていないため、本 issue では扱わない
- V4L2 / MF バックエンドの fps フォールバック (1.0 / 30.0) の変更。既存の契約を維持する
- 解像度の choice 展開（0025 のスコープ外として継続）

## 解決方法

{完了時に記入}
