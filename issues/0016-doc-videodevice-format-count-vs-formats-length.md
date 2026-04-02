# Unix `VideoDevice::format_count` と `formats()` の件数が一致しない場合の説明

Created: 2026-04-02  
Model: Composer 1

## なぜこの対応が必要か

`src/device.rs` の `formats()` は **`video_device_get_format` が NULL を返したインデックスをスキップ**するため、**`formats().len()` が `format_count()` より小さく**なり得る。**利用者が JSON や集計で「件数」を誤解**しないよう、**公開 API の意味を固定**する。

## 現状コード（調査結果）

### `src/device.rs`

- **35〜38 行**: `format_count` → FFI `video_device_format_count` を **`max(0)`**。
- **41〜61 行**: `formats` → **`0..count`** で `get_format`。**NULL なら `continue`**。

### `examples/device_info.rs`

- **54〜58 行**: JSON の **`format_count` は `formats.len()`**（**実際に `Vec` に載った数**）。**`device.format_count()` メソッドとは別**。

### C 実装（参考）

- `video_c.m` / `video_v4l2.c` / `video_pipewire.c` の **`video_device_get_format`** は **範囲外や NULL を返し得る**実装か、**各 issue 側で確認**（**NULL エントリを作らない**ように C を直すのは **別方針**）。

## 提案する実装（この issue は主にドキュメント）

### タスク 1: `VideoDevice` の rustdoc

- **`format_count`**: 「**C が報告するエントリ数**。**`formats()` は NULL をスキップするため、返すベクタの長さがこれより小さい場合がある**」。
- **`formats`**: 「**実際に取得できたフォーマットのリスト**。**`format_count()` と一致しない場合がある**」。

### タスク 2（任意）: `examples/device_info.rs`

- JSON のキー名を **`format_count` → `formats_len`** に変えるか、**`reported_format_count`（C）** と **`resolved_format_count`（Rust）** の二つに分ける。**破壊的変更**になるので **JSON 利用者がいるか**で判断。

### タスク 3（スコープ外の別 issue 向け）

- **C 側で NULL エントリを返さない**ように統一し、**常に件数一致**にする。**作業量が大きい**。

## テスト・検証

- **ドキュメントのみ**の場合: **`cargo doc --no-deps`** で表示確認。
- **JSON 変更**の場合: **既存の出力を利用しているツール**がないか確認。

## 完了条件（チェックリスト）

- [ ] **`VideoDevice::format_count` / `formats`** の rustdoc に、**件数が一致しない理由**が書かれている。
- [ ] `device_info` の JSON を変える場合は **破壊的変更**の説明を issue またはコミットに残す。

## 依存関係

- **なし**。

## 関連ファイル一覧

| ファイル | 変更想定 |
|----------|----------|
| `src/device.rs` | rustdoc |
| `examples/device_info.rs` | 任意 |
