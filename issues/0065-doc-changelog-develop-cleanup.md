# CHANGES.md の develop セクションを規約に合わせる

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-changelog-develop-cleanup
- Polished: {YYYY-MM-DD}

## 目的

`CHANGES.md` の `## develop` セクションに、shiguredo-changelog 規約（派生元ブランチとの最終差分のみ記載する、中間状態の修正は記載しない、種別順を守る）に反する記述がある。

1. `#[ignore]` に実機デバイスが必要な理由を明示する、というエントリが、同じセクション内の「`#[ignore]` ではなく環境変数 `VIDEO_DEVICE_DEVICE_TEST` で判定するようにする」と矛盾している。現コードに `#[ignore]` は無く、中間状態の記述が残っている
2. `Cargo.toml` の windows 依存を MSRV 1.93 の cargo が解釈できる形式に修正する、というエントリが、同じセクションの MSRV 1.95 への更新と矛盾している。MSRV 1.93 は最終状態ではサポート外
3. `Error` への `Copy` 実装は公開 API の追加、`PixelFormat` のメソッドの `self` 受け統一は公開 API の変更であり、`### misc`（機能に直接影響しない変更）ではなく本体セクション（`[ADD]` / `[UPDATE]`）が妥当
4. `raw_player` のバージョン指定を `=2026.2.0` から `2026.2` に緩めた変更が記載されていない。dev-dependency のみの変更を記載しない方針なら、windows 依存の記載との基準を揃える必要がある
5. 冒頭の種別一覧が `UPDATE` → `ADD` → `CHANGE` → `FIX` の順で、エントリの推奨順（`CHANGE` → `ADD` → `UPDATE` → `FIX`）と異なる
6. `### misc` 内のエントリ間の空行が他セクションと不統一

## 優先度根拠

- Low。リリースノートの品質問題であり、ランタイムには影響しない
- 矛盾した記述が残ると、利用者が変更内容を誤解する
- `/review-code` の重要指摘（規約違反）として確認済み

## 現状

`## develop` には `[CHANGE] MSRV を 1.95 に上げる` と `[FIX] Cargo.toml の windows 依存を MSRV 1.93 の cargo が解釈できる形式に修正する` が併記され、`### misc` の末尾に `#[ignore]` のエントリがある。

## 設計方針

- 中間状態のエントリ（`#[ignore]`、MSRV 1.93）を削除する
- 公開 API の追加・変更を本体セクションへ移す
- dev-dependency の記載方針を決め、必要なら `raw_player` の変更を 1 行追加する
- 種別一覧とエントリ順、空行を規約に合わせる

## 完了条件

- `## develop` に矛盾するエントリが無い
- エントリの種別と配置が shiguredo-changelog 規約に適合している
- `## develop` の内容が 2026.3.0 からの最終差分と一致している
- markdownlint で問題が出ない

## 解決方法

{完了時に記入}
