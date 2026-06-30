.PHONY: install
install:
	uv sync --all-extras

.PHONY: lint
lint:
	uv run -- ruff check
	uv run -- ruff format --diff
	uv run -- ty check
	cargo fmt --all --check
	cargo clippy -- -D warnings

.PHONY: setup-development
setup-development:
	rm -rf python/wirio_settings/_wirio_settings.pyi
	uv run -- maturin develop --generate-stubs --uv
	uv run -- maturin generate-stubs -o python/wirio_settings

.PHONY: test
test:
	uv run -- pytest
	cargo test