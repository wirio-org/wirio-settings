<div align="center">
<img alt="Logo" src="https://raw.githubusercontent.com/wirio-org/wirio-settings/refs/heads/main/docs/logo.png" width="450" height="450">

[![CI](https://img.shields.io/github/actions/workflow/status/wirio-org/wirio-settings/ci.yaml?branch=main&logo=github&label=CI)](https://github.com/wirio-org/wirio-settings/actions/workflows/ci.yaml)
[![PyPI - version](https://img.shields.io/pypi/v/wirio-settings?color=blue&label=pypi)](https://pypi.org/project/wirio-settings/)
[![Python - versions](https://img.shields.io/pypi/pyversions/wirio-settings.svg)](https://github.com/wirio-org/wirio-settings)
[![License](https://img.shields.io/github/license/wirio-org/wirio-settings.svg)](https://github.com/wirio-org/wirio-settings/blob/main/LICENSE)

</div>

## Overview

Every Python application — whether it's a simple API, notebook or multi-agent pipeline — needs the same thing under the hood: settings. Model names, URLs, database passwords, timeouts, feature flags. In any real app we have to work with settings — the question is why `wirio-settings`.

Here's why: our application settings, one line, done right. No more scattered `os.environ` calls, no more silent typos in environment variable names, no more manual `.env` parsing — just a typed Pydantic model, loaded from wherever our settings actually live and always up to date.

- **Great defaults from day one:** It automatically looks for settings files and environment variables, with recommended configurations and one line of code.
- **Rust-powered core:** Built with Rust under the hood for speed, reliability, and low runtime overhead.
- **Secret stores:** Azure Key Vault, AWS Secrets Manager and GCP Secret Manager integrations are available with one line of code, with safe authentication.
- **Automatic reloads:** Keep settings up to date by automatically reloading them, with no need to restart the application or deploy a new version.
- **Pydantic models:** Load application settings directly into models.
- **A practical replacement:** Replace `pydantic-settings` and `python-dotenv` with one centralized, provider-agnostic (no vendor lock-in) settings library.
- **Roadmap:** Planned capabilities include pluggable configuration stores, feature flags, prefixes, filters, custom delimiters and aliases.

## Table of contents

- [Overview](#overview)
- [Table of contents](#table-of-contents)
- [📦 Installation](#-installation)
- [🚀 Get started](#-get-started)
  - [1. Introduction](#1-introduction)
  - [2. Read settings](#2-read-settings)
  - [3. Bind the settings to a Pydantic model](#3-bind-the-settings-to-a-pydantic-model)
  - [4. Add environment-specific settings](#4-add-environment-specific-settings)
  - [5. Read the secrets securely](#5-read-the-secrets-securely)
  - [6. Summary](#6-summary)
- [Core concepts](#core-concepts)
  - [Providers and priority](#providers-and-priority)
  - [Default providers](#default-providers)
  - [Environments](#environments)
  - [Key naming and nesting](#key-naming-and-nesting)
  - [Content root](#content-root)
- [Reading settings](#reading-settings)
  - [Read one value](#read-one-value)
  - [Typed values](#typed-values)
  - [Pydantic models](#pydantic-models)
  - [Nested models](#nested-models)
  - [Lists and dictionaries](#lists-and-dictionaries)
  - [Sections](#sections)
- [Recommended usage](#recommended-usage)
- [Providers](#providers)
  - [YAML file](#yaml-file)
  - [JSON file](#json-file)
  - [Environment variables](#environment-variables)
  - [Azure Key Vault](#azure-key-vault)
  - [AWS Secrets Manager](#aws-secrets-manager)
  - [GCP Secret Manager](#gcp-secret-manager)
  - [Setting per file](#setting-per-file)
- [Automatic reloads](#automatic-reloads)
  - [Reload on file change](#reload-on-file-change)
  - [Reload on an interval](#reload-on-an-interval)
  - [Pydantic model reloads](#pydantic-model-reloads)
- [Troubleshooting](#troubleshooting)
  - [Debug settings](#debug-settings)
  - [Common errors](#common-errors)

## 📦 Installation

```bash
uv add wirio-settings
```

## 🚀 Get started

In this mini-tutorial, we configure a small application step by step. Each step builds on the previous one, and the final result is a fully typed settings model that works in local and in production.

### 1. Introduction

We'll use `SettingsManager`, which by default reads:

- Environment variables.
- The `settings.local.yaml` file when it exists, that we'll use for local development.

YAML is a modern alternative to `.env` files that supports typed values and structured settings.

The file name is standardized by well-known frameworks and tools such as Claude Code and GitHub Copilot, enabling environment-specific configuration. As we'll see later, `local` is the environment we use when developing on our machines.

### 2. Read settings

We create a `settings.local.yaml` file in our working directory (it's usually the root of the repository) with the following contents:

```yaml
openai_api_key: secretkey
openai_model: gpt-5
timeout_seconds: 30
postgresql_connection_string: postgresql+asyncpg://user:password@localhost/database
```

> [!WARNING]
> Never commit secrets to version control. This file is for local development only, and it should be ignored in `.gitignore`.

And read the settings using `SettingsManager`:

```python
from wirio_settings import SettingsManager


settings_manager = SettingsManager()

openai_api_key = settings_manager.get_required_value("openai_api_key")
```

Values are returned as strings unless we pass a type as the second argument, which validates and converts the value.

We also can load optional settings with `get_value`, which returns `None` when the setting is missing.

Take into account that, independently of the origin of the setting, it'll always be converted to snake_case because it's the Python convention. For example, the environment variable `POSTGRESQL_CONNECTION_STRING` maps to the key `postgresql_connection_string`.

> [!NOTE]
> If we're comfortable with this simplified approach, or we're prototyping (for example from a Jupyter notebook), we can stop here. The rest of the mini-tutorial is about production-ready practices.

### 3. Bind the settings to a Pydantic model

Reading key by key is fine for a couple of values. For an application, we usually want one validated object instead of using the Magic Strings anti-pattern:

```python
from pydantic import BaseModel
from wirio_settings import SettingsManager


class ApplicationSettings(BaseModel):
    openai_api_key: str
    openai_model: str
    timeout_seconds: int
    postgresql_connection_string: str


application_settings = SettingsManager().get_model(ApplicationSettings)
```

We'll use typed and Pydantic capabilities to express optional values, defaults and nested models.

### 4. Add environment-specific settings

As explained in [Default providers](#default-providers), `SettingsManager` loads `settings.yaml`, `settings.{environment}.yaml`, and environment variables. If the files are missing, they are skipped.

We use different settings per environment. For example, we may want to use a different database connection string, URL or API key in production.

So, we just create a settings file for each environment we want to support. For example:

- `settings.local.yaml` for local development.
- `settings.staging.yaml` for staging.
- `settings.production.yaml` for production.

The environment will be detected (details in [Environments](#environments)) and the proper file will be loaded automatically.

Talking about the `settings.yaml` file, it's used for shared settings that are common to all environments. For example, we may want to use the same OpenAI model in all environments. Using this file we can avoid repeating the same value in all the environment-specific files.

Now our settings are tracked in version control, and we can have different values for each environment without changing the application code or giving developers excessive cloud permissions just to change a setting.

> [!WARNING]
> Never commit secrets to version control. The tracked settings files should contain only non-sensitive values. To load secrets when we deploy (when we're not developing in local), we'll use a secret store (e.g. Azure Key Vault or AWS Secrets Manager) or a different mechanism, as explained in the next section.

### 5. Read the secrets securely

When we're not developing in local, we want to read secrets from a secure location instead of exposing them in a file.

Choose the provider (more in [Providers](#providers)) that matches how the application receives its secrets. Some common providers are:

- [Azure Key Vault](#azure-key-vault) for Azure workloads.
- [AWS Secrets Manager](#aws-secrets-manager) for AWS workloads.
- [GCP Secret Manager](#gcp-secret-manager) for GCP workloads.
- [Setting per file](#setting-per-file) when the runtime mounts secrets as files, such as Docker or Kubernetes secret volumes.
- [Environment variables](#environment-variables) when the deployment platform injects secret values, often through a cloud secret store link or using Kubernetes External Secrets Operator. The application reads the injected value; the platform is responsible for resolving the secret store reference.

For example, we can read the secrets from Azure Key Vault:

```python
class ApplicationSettings(BaseModel):
    openai_api_key: str
    openai_model: str
    timeout_seconds: int
    postgresql_connection_string: str


application_settings = (
    SettingsManager()
    .add_azure_key_vault("https://example.vault.azure.net/")
    .get_model(ApplicationSettings)
)
```

Realize that Azure Key Vault only can store PascalCase or kebab-case secrets, but as they are normalized to snake_case, they're mapped to the Pydantic model fields without any extra code.

> [!NOTE]
> We only need to add the secret store provider when we're not developing in local, so we usually add an `if` statement to check the environment and then adding the provider.

### 6. Summary

We have a single `ApplicationSettings` model that works in local and in production, with no extra code. The settings are loaded from the right provider depending on the environment, and we can add more providers if needed.

The next sections are very important to understand how the settings system works, and they include topics such as the core concepts, how to read values, or the providers themselves.

For a complete recommended usage of how to use `wirio-settings` in production, see [Recommended usage](#recommended-usage).

## Core concepts

### Providers and priority

A provider is a source of settings, such as a YAML file, the environment variables, or a secret store. `wirio-settings` supports multiple providers at the same time, and it merges them into a single flat set of keys.

When the same key exists in several providers, **the last added provider wins**:

```python
settings_manager = SettingsManager()  # Adds the default providers
settings_manager.add_azure_key_vault(  # Overrides the defaults
    "https://example.vault.azure.net/"
)
```

### Default providers

`SettingsManager` adds the following providers, in this order:

1. `settings.yaml`
2. `settings.{environment}.yaml`
3. Environment variables

Considerations:

- The files are optional. If a file is not found, it's skipped.
- The environment variables have a higher priority than the files, because their provider is added last.
- Any provider we add afterwards has a higher priority than all the defaults.

To start from an empty settings manager, disable the defaults:

```python
settings_manager = SettingsManager(add_default_providers=False)
```

We can also add the defaults later with `add_default_providers()`, for example to place them above a provider we added first.

### Environments

By default, `SettingsManager` reads the `WIRIO_ENVIRONMENT` environment variable to determine the environment name, and it defaults to `local` when the variable is not set. For example, `WIRIO_ENVIRONMENT=production` loads `settings.production.yaml`, which is optional.

To use an environment variable with a different name, pass `environment_key`:

```python
settings_manager = SettingsManager(environment_key="PYTHONAPP_ENVIRONMENT")
```

With `PYTHONAPP_ENVIRONMENT=production`, the default providers load `settings.production.yaml`.

### Key naming and nesting

Every provider has its own naming convention, and not every store allows the same characters in a key. `wirio-settings` normalizes all of them into the same shape:

- Keys are converted to snake case. `APP_NAME`, `appName`, `AppName`, and `app-name` all map to `app_name`.
- Sections are separated with `.`, as in `database.host` or `logging.log_level.default`.
- Each provider declares how sections are written in its own store. For example:

  | Provider                            | Section separator | Example               | Setting key     |
  | ----------------------------------- | ----------------- | --------------------- | --------------- |
  | YAML file, JSON file                | Nested objects    | `database: {host: …}` | `database.host` |
  | Environment variables               | `__`              | `DATABASE__HOST`      | `database.host` |
  | Azure Key Vault, GCP Secret Manager | `--`              | `Database--Host`      | `database.host` |
  | AWS Secrets Manager                 | Nested JSON       | `{"database": {…}}`   | `database.host` |
  | Setting per file                    | None              | `database.host` file  | `database.host` |

Sequences are flattened with their index, so the first item of the `servers` list is `servers.0`.

### Content root

Relative file paths are resolved against the content root, which is the current working directory by default. To resolve them against another directory, pass an absolute `content_root_path`:

```python
settings_manager = SettingsManager(content_root_path="/opt/orders-api")
```

## Reading settings

### Read one value

Use `get_required_value` when the key must exist. It raises a `KeyError` when the key is missing:

```python
openai_api_key = settings_manager.get_required_value("openai_api_key")
```

Use `get_value` for optional keys. It returns `None` when the key is missing:

```python
openai_api_key = settings_manager.get_value("openai_api_key")
```

### Typed values

By default, the settings system returns values as strings. To validate and convert to another type, pass the type as a second argument:

```python
maximum_retries = settings_manager.get_required_value("maximum_retries", int)
enable_cache = settings_manager.get_value("enable_cache", bool)
```

The conversion is done internally by Pydantic, so an invalid value raises a validation error. Lists and dictionaries are read from several keys, so they are best read through a [model](#lists-and-dictionaries) instead of a single value.

### Pydantic models

`get_model` builds a model from the settings, mapping each field name to a setting key:

```python
from pydantic import BaseModel
from wirio_settings import SettingsManager


class ApplicationSettings(BaseModel):
    app_name: str
    port: int | None = None


application_settings = SettingsManager().get_model(ApplicationSettings)
```

- If a field has a default, that default is used when no value is found. Here, `port` defaults to `None` when missing.
- If a required field is missing, `get_model` raises a `KeyError`.

### Nested models

A field annotated with another model is bound to the section with the same name:

```yaml
database:
  host: localhost
  port: 5432
```

```python
class DatabaseSettings(BaseModel):
    host: str
    port: int


class ApplicationSettings(BaseModel):
    database: DatabaseSettings
```

### Lists and dictionaries

Lists are read from indexed keys, and dictionaries are read from the children of a section. Both work with scalars and with models:

```yaml
ports:
  - 8080
  - 8081

servers:
  - name: api
    retries: 3
  - name: worker

services:
  api:
    url: https://api.example.com
  worker:
    url: https://worker.example.com
```

```python
class Server(BaseModel):
    name: str
    retries: int = 3


class Service(BaseModel):
    url: str


class ApplicationSettings(BaseModel):
    ports: list[int]
    servers: list[Server]
    services: dict[str, Service]
```

### Sections

Use `get_section` to read a group of settings that share a prefix. For example, we can read the next YAML:

```yaml
logging:
  log_level: WARNING
```

```python
logging_section = settings_manager.get_section("logging")
log_level = logging_section.get_required_value("log_level")
```

A section behaves like the settings manager itself, so it supports getting values, subsections and Pydantic models.

```python
logging_settings = settings_manager.get_section("logging").get_model(LoggingSettings)
```

`get_section` raises a `KeyError` when the key is not a section.

## Recommended usage

If we use environment variables for sensitive and non-sensitive settings, we don't have to do anything.

```python
application_settings = SettingsManager().get_model(ApplicationSettings)
```

But we should read non-sensitive settings from settings files (`settings.{environment}.yaml`) tracked in version control.

To do that, we can add the `WIRIO_ENVIRONMENT` environment variable to the deployed application. For example, `WIRIO_ENVIRONMENT=production`, and the `settings.production.yaml` file will be loaded automatically.

> [!NOTE]
> If we want to use another environment variable, we can pass `environment_key` to `SettingsManager`, as explained in [Environments](#environments).

Now, we have all the pieces in place, but some of the integrations should only be activated when the application is deployed. In local, we don't want to touch secret stores, instrument libraries, send telemetry to the cloud, use HSTS, add CORS, enable caching, use some authentication mechanisms, etc. so we have to add a simple environment check.

This might sound like an extra layer of complexity, but it's what we must do independently of the settings library we use.

For example, if we use the `WIRIO_ENVIRONMENT` environment variable to detect the current environment, we can add a secret volume in this way:

```python
from os

from fastapi import FastAPI
from wirio_settings import SettingsManager


settings_manager = SettingsManager()

if os.getenv("WIRIO_ENVIRONMENT", "local") != "local":
    settings_manager.add_setting_per_file("/run/secrets")

    # Enable telemetry, etc.

application_settings = settings_manager.get_model(ApplicationSettings)
app = FastAPI()
```

## Providers

### YAML file

```python
settings_manager.add_yaml_file("settings.yaml")
```

Comments are supported in YAML files. The filename may be a relative path, such as `../settings.yaml`, which is resolved against the [content root](#content-root). Absolute paths are used as they are.

Options:

- `optional=True` skips the file if it is missing. The file is required by default.
- `reload_on_change=True` reloads values when the file changes.

### JSON file

```python
settings_manager.add_json_file("settings.json")
```

Comments are not supported in JSON files. The filename may be a relative path, such as `../settings.json`, which is resolved against the [content root](#content-root). Absolute paths are used as they are.

Options:

- `optional=True` skips the file if it is missing. The file is required by default.
- `reload_on_change=True` reloads values when the file changes.

### Environment variables

```python
settings_manager.add_environment_variables()
```

Keys are normalized to snake case, and `__` is replaced with `.`. For example, `DATABASE__HOST` maps to `database.host`.

### Azure Key Vault

```python
settings_manager.add_azure_key_vault(
    "https://example.vault.azure.net",
)
```

Secret names use `--` for sections, so `Database--Host` maps to `database.host`.

If no explicit credentials are provided, `DefaultAzureCredential` is used.

`DefaultAzureCredential` tries credentials in this order and uses the first one that succeeds:

1. Environment credential (`AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`)
2. Workload identity credential
3. Developer tools credential (Azure CLI / Azure Developer CLI)
4. Managed identity credential. This is the System-assigned managed identity by default. If we want to use a User-assigned managed identity, set the `AZURE_CLIENT_ID` environment variable.

To use explicit service principal credentials, provide all three values:

```python
settings_manager.add_azure_key_vault(
    "https://example.vault.azure.net",
    client_id="...",
    client_secret="...",
    tenant_id="...",
)
```

When using explicit credentials, `tenant_id`, `client_id`, and `client_secret` must all be provided.

> [!NOTE]
> **Azure permissions:** Usually, the `Key Vault Secrets User` role is used to read secrets.

To periodically refresh the loaded secrets, use the `reload_interval` parameter, described in [Reload on an interval](#reload-on-an-interval).

### AWS Secrets Manager

```python
settings_manager.add_aws_secrets_manager(
    "secret-id",
)
```

The secret value must be a JSON object. `wirio-settings` reads and flattens that JSON into settings keys.

By default, the provider uses the [credential provider chain](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/credproviders.html#credproviders-default-credentials-provider-chain). For example, the IAM role, the shared AWS configuration profile, or `AWS_*` environment variables.

If explicit credentials are provided, they override environment authentication for this provider instance:

```python
settings_manager.add_aws_secrets_manager(
    secret_id="secret-id",
    access_key_id="...",
    secret_access_key="...",
)
```

Options:

- `region` selects the AWS region.
- `profile` selects a shared configuration profile.
- `session_token` is used together with temporary credentials.
- `url` overrides the service endpoint, which is useful when testing against a local emulator.

### GCP Secret Manager

```python
settings_manager.add_gcp_secret_manager("project-id")
```

Secret names use `--` for sections, so `Database--Host` maps to `database.host`.

If no credentials are provided, [Application Default Credentials (ADC)](https://docs.cloud.google.com/docs/authentication/application-default-credentials) are used.
We can also pass custom GCP credentials with the `credentials_json` parameter.

### Setting per file

```python
settings_manager.add_setting_per_file("/run/secrets")
```

Given a directory, each file name becomes a setting key and the file content becomes the setting value. The directory path must be absolute, because it is not resolved against the [content root](#content-root).

Options:

- `optional=True` skips the directory if it is missing. The directory is required by default.
- `reload_on_change=True` reloads values when directory contents change.

This provider is useful when secrets are mounted as files by the runtime instead of exposed as environment variables. It lets us keep application code unchanged while switching the secret delivery mechanism.

Common use cases:

- Kubernetes with [Secrets Store CSI Driver](https://secrets-store-csi-driver.sigs.k8s.io/) where providers such as Azure Key Vault mount each secret as a file into a volume.
- Docker secret mounts (for example, `/run/secrets`).
- Platform-managed secret volumes in production environments where file-based delivery is preferred.

Example directory:

```
/run/secrets/
    database_password
    openai_api_key
```

Then the values are available as `database_password` and `openai_api_key`.

This provider does not translate any separator, so the file name is used as the setting key. To read a nested key, include the `.` in the file name, as in `database.host`.

## Automatic reloads

Long-running applications, such as web servers or background jobs, can keep their settings up to date without restarting or redeploying.

### Reload on file change

The file and directory providers watch their source when `reload_on_change=True`:

```python
settings_manager.add_yaml_file("settings.yaml", reload_on_change=True)
```

### Reload on an interval

Azure Key Vault refreshes its secrets in the background when `reload_interval` is set. The provider waits that long between refresh attempts, and it keeps the last successfully loaded settings if a refresh fails:

```python
from datetime import timedelta


settings_manager.add_azure_key_vault(
    "https://example.vault.azure.net",
    reload_interval=timedelta(minutes=5),
)
```

### Pydantic model reloads

Models returned by `get_model()` are automatically updated when a provider reloads its values, so there is no need to call `get_model()` again:

```python
from pydantic import BaseModel
from wirio_settings import SettingsManager


class ApplicationSettings(BaseModel):
    port: int


application_settings = (
    SettingsManager()
    .add_yaml_file("settings.yaml", reload_on_change=True)
    .get_model(ApplicationSettings)
)
```

When `settings.yaml` changes its contents, `application_settings.port` is updated in place. If the refreshed values do not validate against the model, the existing model values are retained.

## Troubleshooting

### Debug settings

Use `debug_repr()` to inspect settings and their providers. When several providers contain the same key, the value from the provider with the highest priority is shown.

```python
print(settings_manager.debug_repr())
```

### Common errors

| Error                                           | Cause                                                                                      |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `KeyError: Missing setting value for key '…'`   | No provider contains the key. Check the spelling, the section separator, and the priority. |
| `ValueError: Setting value for key '…' is None` | The key exists, but it has no value. For example, a YAML key declared without a value.     |
| `KeyError: Setting key '…' is not a section`    | `get_section` was called with a key that has no children.                                  |
| A validation error from Pydantic                | The value exists but it cannot be converted to the requested type.                         |
