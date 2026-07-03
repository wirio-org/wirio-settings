<div align="center">
<img alt="Logo" src="https://raw.githubusercontent.com/wirio-org/wirio-settings/refs/heads/main/docs/logo.png" width="450" height="450">

[![CI](https://img.shields.io/github/actions/workflow/status/wirio-org/wirio-settings/ci.yaml?branch=main&logo=github&label=CI)](https://github.com/wirio-org/wirio-settings/actions/workflows/ci.yaml)
[![PyPI - version](https://img.shields.io/pypi/v/wirio-settings?color=blue&label=pypi)](https://pypi.org/project/wirio-settings/)
[![Python - versions](https://img.shields.io/pypi/pyversions/wirio-settings.svg)](https://github.com/wirio-org/wirio-settings)
[![coverage](https://codecov.io/gh/wirio-org/wirio-settings/graph/badge.svg)](https://codecov.io/gh/wirio-org/wirio-settings)
[![License](https://img.shields.io/github/license/wirio-org/wirio-settings.svg)](https://github.com/wirio-org/wirio-settings/blob/main/LICENSE)

</div>

## Overview

Lightning-fast, strongly typed, and zero boilerplate settings library for Python:

- **Great defaults from day one:** It automatically looks for settings files and environment variables, with recommended configurations and zero boilerplate.
- **Rust-powered core:** Built with Rust under the hood for speed, reliability, and low runtime overhead.
- **Cloud secrets when we need them:** Azure Key Vault, AWS Secrets Manager and GCP Secret Manager integrations are available with one line of code, with safe authentication.
- **Works naturally with Pydantic models:** Load your app settings directly into models.
- **Async-ready by design:** Built to work smoothly with modern async Python apps and cloud SDKs.
- **A practical replacement:** Replace `pydantic-settings` and `python-dotenv` with one unified settings library.
- **Roadmap:** Planned capabilities include automatic reload on settings changes (events, sentinels, time, async), pluggable configuration stores, feature flags, lifetimes, prefixes, custom delimiters and aliases.

## 📦 Installation

```bash
uv add wirio-settings
```

## ✨ Quickstart with magic strings

```python
from wirio_settings import SettingsManager


settings_manager = SettingsManager()
database_password = settings_manager.get_required_value("database_password")
```

## ✨ Quickstart with Pydantic models

```python
from pydantic import BaseModel
from wirio_settings import SettingsManager


class ApplicationSettings(BaseModel):
    database_password: str


application_settings = SettingsManager().get_model(ApplicationSettings)
```

## ✨ Quickstart with Pydantic models and Azure Key Vault

```python
from pydantic import BaseModel
from wirio_settings import SettingsManager


class ApplicationSettings(BaseModel):
    database_password: str


application_settings = (
    SettingsManager()
    .add_azure_key_vault("https://example.vault.azure.net/")
    .get_model(ApplicationSettings)
)
```

## Source priority

`wirio-settings` supports multiple sources. When the same key exists in multiple sources, the last added sources have more priority.

The following sources are loaded, by default, in this order:

1. `settings.yaml`
2. `settings.{environment}.yaml`.
3. Environment variables

Considerations:

- Files are optional. If a file is not found, it's skipped.
- `{environment}` is the value of the `WIRIO_ENVIRONMENT` environment variable. If the variable is not set, its value is `local`. This would load, for example, `settings.production.yaml` if `WIRIO_ENVIRONMENT=production`. It standardizes the environment detection and allows us to store all settings in code, with version control.
- In the default settings, environment variables have higher priority than YAML files because the source is added after them. This means that if a key exists in both `settings.yaml` and environment variables, the value from environment variables will be used.
- If we add more sources, those will have higher priority than the defaults. For example, if we add Azure Key Vault as a source, it will override the defaults.

  ```python
  SettingsManager().add_azure_key_vault("https://example.vault.azure.net/")
  ```

## Naming convention

Each source (environment variables, YAML, Azure Key Vault...) has its own naming convention for keys. `wirio-settings` uses snake case for settings keys. When loading from sources, keys are normalized to snake case. For example, the `APP_NAME` environment variable maps to `app_name`.

## Recommended usage

It depends on your usage, but the recommended setup is having the following files:

- `settings.yaml` with the shared settings.
- `settings.{environment}.yaml` with the environment-specific settings. For example, `settings.production.yaml`, `settings.staging.yaml`, `settings.local.yaml`, etc.

Then, we declare the settings manager, loading the default sources:

```python
settings_manager = SettingsManager()
```

Now, `settings_manager` has settings loaded.

Let's say our API is deployed in production, so we have read the `key_vault_url` setting from `settings.production.yaml`. We can now add Azure Key Vault as a source and read the rest of the settings from there:

```python
settings_manager.add_azure_key_vault(
    settings_manager.get_required_value("key_vault_url")
)
```

After that, we have all settings to construct our Pydantic model:

```python
application_settings = settings_manager.get_model(ApplicationSettings)
```

## Read one value

- Use `get_required_value` when the key must exist.

```python
openai_api_key = settings_manager.get_required_value("openai_api_key")
```

By default, the settings system returns values as strings. To validate and convert to another type, pass the type as a second argument.

```python
timeout_seconds = settings_manager.get_required_value("maximum_retries", int)
```

- Use `get_value` for optional keys.

```python
openai_api_key = settings_manager.get_value("openai_api_key")
timeout_seconds = settings_manager.get_value("maximum_retries", int)
```

## Defaults and required fields

If a model field has a default, that default is used when no value is found.

```python
from pydantic import BaseModel


class ApplicationSettings(BaseModel):
    app_name: str
    port: int | None = None
```

Here, `port` defaults to `None` when missing.
If a required field is missing, `get_model` raises `KeyError`.

## Sections

Use `get_section` to read a section. For example, we can read the next YAML:

```yaml
logging:
  log_level: WARNING
```

```python
log_level = settings_manager.get_section("logging").get_required_value(
    "log_level"
)
```

`SettingsSection` supports:

- The section value itself with `section.get_required_value()` or `section.get_required_value(type)`.
- A child value with `section.get_required_value("child.key")` or `section.get_required_value("child.key", type)`.

If a section has only children and no value at its own path, `section.get_value()` returns `None`.

## Nested keys

Nested keys use `.`:

- `database.host`
- `database.port`
- `logging.log_level.default`

## All sources

### YAML file

```python
settings_manager.add_yaml_file("file.yaml")
```

Comments are supported in YAML files.

### JSON file

```python
settings_manager.add_json_file("file.json")
```

Comments are not supported in JSON files.

### Environment variables

```python
settings_manager.add_environment_variables()
```

### Azure Key Vault

```bash
uv add wirio-settings[azure-key-vault]
```

```python
settings_manager.add_azure_key_vault(
    "https://example.vault.azure.net",
)
```

If no credential is provided, `DefaultAzureCredential` is used.
We can also pass a custom async Azure credential with the `credential` parameter.

### AWS Secrets Manager

```bash
uv add wirio-settings[aws-secrets-manager]
```

```python
settings_manager.add_aws_secrets_manager(
    "SecretName"
)
```

The secret value must be a JSON object. `wirio-settings` reads and flattens that JSON into settings keys.

### GCP Secret Manager

```bash
uv add wirio-settings[gcp-secret-manager]
```

```python
settings_manager.add_gcp_secret_manager(
    "project-id"
)
```

If no credentials are provided, Application Default Credentials (ADC) are used.
We can also pass custom GCP credentials with the `credentials` parameter.

## Contributing

### Generate test coverage report

```bash
rustup component add llvm-tools-preview
```

```bash
cargo install cargo-llvm-cov
```

```bash
make test-coverage
```
