.PHONY: test cover pbt-with-cover check clippy fmt clean

# 全テストを実行する
test:
	cargo test --workspace --all-targets

# 全テストカバレッジ付きで実行する
cover:
	cargo llvm-cov --tests --workspace

# PBT をカバレッジ付きで実行する
pbt-with-cover:
	cargo llvm-cov -p pbt --tests

# cargo check を実行する
check:
	cargo check --workspace --all-targets

# cargo clippy を実行する
clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# cargo fmt を実行する
fmt:
	cargo fmt --all

# ビルド成果物を削除する
clean:
	cargo clean
