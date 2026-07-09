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

.PHONY: generate-stubs
generate-stubs:
	rm -rf python/wirio_settings/_wirio_settings.pyi
	uv run -- maturin develop --generate-stubs --uv
	uv run -- maturin generate-stubs -o python/wirio_settings

.PHONY: test
test:
	uv run -- pytest
	-deactivate
	cargo test

# Prerequisite: aws login
.PHONY: aws-secrets-manager-integration-test
aws-secrets-manager-integration-test:
	aws secretsmanager delete-secret --secret-id "dev/test-secret-id" --force-delete-without-recovery
	aws secretsmanager create-secret --name "dev/test-secret-id" --secret-string '{"secret_1":"secret-value-1","Secret-2":"secret-value-2","parent":{"nestedSecret":"Nested-value"}}'
	INTEGRATION_TEST=1 uv run -- pytest tests/test_integration.py::TestIntegration::test_load_secret_using_aws_secrets_manager
	aws secretsmanager delete-secret --secret-id "dev/test-secret-id" --force-delete-without-recovery


.PHONY: gcp-secret-manager-integration-test
gcp-secret-manager-integration-test:
	-gcloud secrets delete "secret_1" --quiet
	-gcloud secrets delete "Secret-2" --quiet
	-gcloud secrets delete "parent--nestedSecret" --quiet
	printf '%s' "secret-value-1" | gcloud secrets create "secret_1" --data-file=-
	printf '%s' "secret-value-2" | gcloud secrets create "Secret-2" --data-file=-
	printf '%s' "Nested-value" | gcloud secrets create "parent--nestedSecret" --data-file=-
	INTEGRATION_TEST=1 GCP_PROJECT_ID="$$(gcloud config get-value project --quiet)" uv run -- pytest tests/test_integration.py::TestIntegration::test_load_secret_using_gcp_secret_manager
	gcloud secrets delete "secret_1" --quiet
	gcloud secrets delete "Secret-2" --quiet
	gcloud secrets delete "parent--nestedSecret" --quiet

.PHONY: test-coverage
test-coverage:
	cargo llvm-cov test --all-features --html
	open ./target/llvm-cov/html/index.html