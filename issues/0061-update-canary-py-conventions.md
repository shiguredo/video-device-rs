# canary.py のタグ種別・ブランチ検証と規約違反を修正する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-canary-py-conventions
- Polished: {YYYY-MM-DD}

## 目的

`canary.py` に、タグ付けと規約に関する問題がある。

1. `git tag` に `-a` を付けておらず軽量タグを作る。既存のリリースタグ（例: 過去のバージョンタグ）は注釈付きタグであり、canary タグだけ種別が異なる
2. `git push` を現在のブランチに対して無条件に実行する。develop 以外のブランチで実行すると、意図しないブランチとタグが公開される
3. 関数のコメントが実態と合っていない（`git_commit_version` の上に「git コミット、タグ、プッシュを実行」と書かれている、`git_operations_after_build` はビルドを行わない）
4. `count=1,  # 最初の1つだけを更新` の末尾コメントは shiguredo-python の「末尾コメントを利用しないこと」に違反。さらに「最初の1つ」の全角・半角間に半角スペースが無い
5. `raise ValueError("Failed to extract the new version after the update.")` はエラーメッセージ末尾のピリオドが規約違反
6. `Optional[str]` はサポート対象 Python（3.12〜3.14）では `str | None` が適切

## 優先度根拠

- Low。リリース補助スクリプトの品質問題であり、通常の開発フローには影響しない
- 誤ったブランチからの push は実害がありうる
- `/review-code` の指摘として確認済み

## 現状

```python
subprocess.run(["git", "tag", new_version], check=True)
subprocess.run(["git", "push"], check=True)
subprocess.run(["git", "push", "origin", new_version], check=True)
```

`git rev-parse --abbrev-ref HEAD` などによるブランチ検証は無い。

## 設計方針

- `git tag -a -m <message>` に統一する
- push 前に現在のブランチが develop であることを検証し、異なる場合は中断する
- 関数コメントを実態に合わせ、`git_operations_after_build` を実態に合う名前に変更する
- 末尾コメントを直前の行に移し、全角半角の間へ半角スペースを入れる
- エラーメッセージの末尾ピリオドを削除する
- `Optional[str]` を `str | None` に統一する

## 完了条件

- canary タグが注釈付きタグになる
- develop 以外のブランチで実行すると push 前に中断する
- コメント・エラーメッセージ・型注釈が shiguredo-python 規約に適合する
- `python3 -m py_compile canary.py` が通る
- `--dry-run` で既存の動作確認ができる

## 解決方法

{完了時に記入}
