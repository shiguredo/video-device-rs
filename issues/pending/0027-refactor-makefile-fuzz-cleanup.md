# Makefile の fuzz 系ターゲットが `fuzz/` ディレクトリ不在で死にコードになっている

- Priority: Low
- Created: 2026-06-15
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/refactor-makefile-fuzz-cleanup
- Polished: 2026-06-15

## 目的

`Makefile` の `fuzzing` / `fuzzing-parallel` / `fuzzing-list` ターゲットは `cargo fuzz` と `fuzz/` ディレクトリを前提としているが、リポジトリには `fuzz/` ディレクトリも `Cargo.toml` の `[package.metadata.fuzz]` も存在しない。死にコードとして残り続けており、利用者が `make fuzzing` 等を叩いても期待した動作にならない。加えて `.PHONY` 宣言 (`Makefile:1`) には実体のないターゲット名 (`pbt` / `pbt-cover` / `fuzz`) が並んでいる一方、実体のあるターゲット `pbt-with-cover` は `.PHONY` に含まれていない。これらの不整合をまとめて整理する。

## 優先度根拠

- Low。実害は少ない (fuzz を叩こうとした開発者が `cargo fuzz list` の空応答に遭遇する程度)
- ただし AGENTS.md「Don't live with broken windows」「If it hurts, do it more often」に直接違反する死にコードであり、放置すべきでない
- `/review-code` で死にコードとして指摘されているが、実害の度合いから本 issue の優先度は Low

## 現状

`Makefile:1` の `.PHONY` 宣言:

```makefile
.PHONY: test cover pbt pbt-cover fuzz fuzzing fuzzing-parallel fuzzing-list check clippy fmt clean
```

`.PHONY` 列挙ターゲットとレシピ定義の対応:

- レシピあり、`.PHONY` あり (正常): `test` / `cover` / `fuzzing` / `fuzzing-parallel` / `fuzzing-list` / `check` / `clippy` / `fmt` / `clean`
- レシピあり、`.PHONY` なし (要追加): `pbt-with-cover` (`Makefile:12-13`)
- レシピなし、`.PHONY` あり (要削除): `pbt` / `pbt-cover` / `fuzz`

`Makefile:16-42` の fuzz 系レシピは以下のように `cargo fuzz list` および `fuzz/` ディレクトリに依存する:

```makefile
fuzzing:
	@FORKS=$$(( $$(nproc) - 2 )); \
	if [ $$FORKS -lt 1 ]; then FORKS=1; fi; \
	echo "Using fork=$$FORKS on $$(nproc) cores"; \
	for target in $$(cargo fuzz list); do \
		echo "=== Fuzzing $$target ==="; \
		cargo +nightly fuzz run $$target -- -max_total_time=30 -fork=$$FORKS -max_len=4096 || exit 1; \
	done

fuzzing-parallel:
	@mkdir -p fuzz/logs
	@FORKS=$$(( $$(nproc) - 2 )); \
	...
	cargo fuzz list | xargs -P $$(cargo fuzz list | wc -l) -I {} \
		sh -c 'cargo +nightly fuzz run {} -- -max_total_time=30 -fork=1 -max_len=4096 > fuzz/logs/{}.log 2>&1'
	...

fuzzing-list:
	cargo fuzz list
```

リポジトリ確認:

- `fuzz/` ディレクトリ: 不在
- `Cargo.toml` に `[package.metadata.fuzz]` 等の cargo-fuzz 設定: 無し
- 結果として `cargo fuzz list` は空応答、`make fuzzing` / `make fuzzing-parallel` は何もせずに終了する
- 加えて `fuzzing-parallel` の `xargs -P $$(cargo fuzz list | wc -l)` は `wc -l` が 0 になるため `xargs -P 0` (制限なし) として動く可能性があり、不意の挙動につながりうる

## 設計方針

死にコードを **削除する**。`cargo fuzz` を整備するかは別の検討事項であり、本 refactor issue のスコープから外す。

### 採用しない案

- **fuzz を整備する**: `cargo fuzz init` で `fuzz/` を生成し、`frame_math::nv12_plane_sizes` 等への純粋関数 fuzz target を追加する案。フレーム計算ロジックの境界値検証は強化されるが、fuzz target 設計と nightly toolchain 維持という新たな責務が発生する。これは本 refactor の単一目的 (死にコード削除) を超えるため、別 issue (add カテゴリ、例: `add-cargo-fuzz-frame-math`) で扱う

## 具体的な変更

1. `Makefile:1` の `.PHONY` から以下のターゲット名を削除する: `pbt` / `pbt-cover` / `fuzz` / `fuzzing` / `fuzzing-parallel` / `fuzzing-list`
2. `Makefile:1` の `.PHONY` に `pbt-with-cover` を追加する (実体は `Makefile:12-13` に既に存在)。`pbt-with-cover` は workspace member `pbt/` のカバレッジ計測ターゲットで、`Cargo.toml` の `[workspace] members = ["pbt"]` が示すとおり今後も維持する
3. `Makefile:16-42` の `fuzzing` / `fuzzing-parallel` / `fuzzing-list` レシピ定義 (各レシピを区切る空行を含む) を削除する

修正後の `.PHONY` 宣言は次の形になる:

```makefile
.PHONY: test cover pbt-with-cover check clippy fmt clean
```

`check` / `clippy` / `fmt` / `clean` のレシピは触らない。

## 影響範囲

- `Makefile`: `.PHONY` 行の書き換えと `fuzzing` / `fuzzing-parallel` / `fuzzing-list` レシピ削除 (合計 27 行ほどの削除)
- ソースコード (`src/`, `tests/`, `pbt/`, `examples/`): 変更なし
- 公開 API: 変更なし
- 既存の `make test` / `make cover` / `make pbt-with-cover` / `make check` / `make clippy` / `make fmt` / `make clean` の挙動は不変
- 既存の `make fuzzing` / `make fuzzing-parallel` / `make fuzzing-list` は **無くなる** (もともと意味のある結果を返していなかったため利用者影響なし)

## 完了条件

- `Makefile` から `fuzzing` / `fuzzing-parallel` / `fuzzing-list` レシピを削除する
- `Makefile:1` の `.PHONY` を `test cover pbt-with-cover check clippy fmt clean` に揃える
- `make test` / `make cover` / `make pbt-with-cover` / `make check` / `make clippy` / `make fmt` / `make clean` がいずれも引き続き正常実行できることを手元で確認する
- `make fuzzing` / `make fuzzing-parallel` / `make fuzzing-list` がターゲット未定義により非ゼロ終了することを確認する (死にコードが消えたことの裏付け。`make` のエラー文言はロケール / 実装で異なるため文言は問わない)
- `CHANGES.md` の `## develop` の `### misc` セクションに以下のエントリを追記する。担当者行 (`- @<github-id>`) を含める。Makefile は公開 API に影響しない開発ツールのため `### misc` に置き、ターゲット削除は手元コマンドの後方互換を破る変更なので `[CHANGE]` ラベルを使う

  ```markdown
  - [CHANGE] `Makefile` から `fuzz/` ディレクトリ未整備で動作しない fuzz 系ターゲット (`fuzzing` / `fuzzing-parallel` / `fuzzing-list`) を削除する
    - @<github-id>
  ```

## pending 理由

わざわざ削除するほどのものでもないので pending とする。

## 解決方法

{完了時に記入}
