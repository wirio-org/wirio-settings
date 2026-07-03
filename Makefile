.PHONY: install
install:
	uv sync --all-extras

.PHONY: lint
lint:
	uv run --locked -- ruff check
	uv run -- ruff format --diff
	uv run -- ty check
	cargo fmt --all --check
	cargo clippy --locked -- --deny warnings

.PHONY: setup-development
setup-development:
	rm -rf python/wirio_settings/_wirio_settings.pyi
	uv run -- maturin develop --generate-stubs --uv
	uv run -- maturin generate-stubs -o python/wirio_settings

.PHONY: test
test:
	uv run -- pytest
	-deactivate
	cargo test

.PHONY: test-coverage
test-coverage:
	cargo llvm-cov test --all-features --html
	open ./target/llvm-cov/html/index.html