.PHONY: install
install:
	uv sync --all-extras

.PHONY: check-code
check-code:
	uv run -- ruff check
	uv run -- ruff format --diff
	uv run -- ty check

.PHONY: setup-development
setup-development:
	uv run -- maturin develop --generate-stubs --uv
	uv run -- maturin generate-stubs -o python/wirio_settings

