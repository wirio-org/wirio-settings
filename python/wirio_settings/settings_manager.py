from os import environ
from typing import Final, Self, cast, final, override

from pydantic import TypeAdapter

from wirio_settings._wirio_settings import SettingLookup, SettingsPath, SettingsProvider
from wirio_settings.aws_secrets_manager.aws_secrets_manager_settings_source import (
    AwsSecretsManagerSettingsSource,
)
from wirio_settings.azure_key_vault.azure_key_vault_settings_source import (
    AzureKeyVaultSettingsSource,
)
from wirio_settings.core._typed_type import TypedType
from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_root import SettingsRoot
from wirio_settings.core.settings_section import SettingsSection
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.environment_variables.environment_variables_settings_source import (
    EnvironmentVariablesSettingsSource,
)
from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_source import (
    GcpSecretManagerSettingsSource,
)
from wirio_settings.json_file.json_file_settings_source import JsonFileSettingsSource
from wirio_settings.key_per_file.key_per_file_settings_source import (
    KeyPerFileSettingsSource,
)
from wirio_settings.yaml_file.yaml_file_settings_source import YamlFileSettingsSource


@final
class SettingsManager(SettingsBuilder, SettingsRoot):
    _content_root_path: Final[str | None]
    _sources: Final[list[SettingsSource]]
    _providers: Final[list[SettingsProvider]]

    def __init__(
        self, content_root_path: str | None = None, add_default_providers: bool = True
    ) -> None:
        """Initialize the settings manager.

        Args:
            content_root_path: Absolute path to the directory that contains the application content files.
                If not provided, the content root path will be determined automatically based on the current working directory.
            add_default_providers: Whether to add the default settings providers.

        """
        self._content_root_path = content_root_path
        self._sources = []
        self._providers = []

        if add_default_providers:
            self.add_default_providers()

    @property
    def sources(self) -> list[SettingsSource]:
        return self._sources

    @property
    @override
    def providers(self) -> list[SettingsProvider]:
        return self._providers

    def add(self, source: SettingsSource) -> None:
        self._sources.append(source)
        provider = source.build(self)
        provider.load_sync()
        self._providers.append(provider)

    def add_default_providers(self) -> Self:
        """Add default settings providers in the recommended order."""
        environment_name = environ.get("WIRIO_ENVIRONMENT", "local")
        return (
            self.add_yaml_file("settings.yaml", optional=True)
            .add_yaml_file(
                f"settings.{environment_name}.yaml",
                optional=True,
            )
            .add_environment_variables()
        )

    def add_environment_variables(self) -> Self:
        """Add a settings provider that reads settings values from environment variables."""
        self.add(EnvironmentVariablesSettingsSource())
        return self

    def add_yaml_file(self, path: str, optional: bool = False) -> Self:
        """Add a settings provider that reads settings values from a YAML file."""
        self.add(
            YamlFileSettingsSource(
                content_root_path=self._content_root_path, path=path, optional=optional
            )
        )
        return self

    def add_json_file(self, path: str, optional: bool = False) -> Self:
        """Add a settings provider that reads settings values from a JSON file."""
        self.add(
            JsonFileSettingsSource(
                content_root_path=self._content_root_path, path=path, optional=optional
            )
        )
        return self

    def add_key_per_file(self, directory_path: str, optional: bool = False) -> Self:
        """Add settings using files from a directory. File names are used as the key, file contents are used as the value."""
        self.add(
            KeyPerFileSettingsSource(
                directory_path=directory_path,
                optional=optional,
            )
        )
        return self

    def add_azure_key_vault(
        self,
        url: str,
        client_id: str | None = None,
        client_secret: str | None = None,
        tenant_id: str | None = None,
    ) -> Self:
        """Add a settings provider that reads settings values from Azure Key Vault."""
        self.add(
            AzureKeyVaultSettingsSource(
                url=url,
                client_id=client_id,
                client_secret=client_secret,
                tenant_id=tenant_id,
            )
        )
        return self

    def add_aws_secrets_manager(  # noqa: PLR0913
        self,
        secret_id: str,
        region: str | None = None,
        url: str | None = None,
        access_key_id: str | None = None,
        secret_access_key: str | None = None,
        session_token: str | None = None,
        profile: str | None = None,
    ) -> Self:
        """Add a settings provider that reads settings values from AWS Secrets Manager."""
        self.add(
            AwsSecretsManagerSettingsSource(
                secret_id=secret_id,
                region=region,
                url=url,
                access_key_id=access_key_id,
                secret_access_key=secret_access_key,
                session_token=session_token,
                profile=profile,
            )
        )
        return self

    def add_gcp_secret_manager(
        self,
        project_id: str,
        credentials_json: str | None = None,
    ) -> Self:
        """Add a settings provider that reads settings values from GCP Secret Manager."""
        self.add(
            GcpSecretManagerSettingsSource(
                project_id=project_id,
                credentials_json=credentials_json,
            )
        )
        return self

    @override
    def get_value[TField = str](
        self,
        key: str,
        value_type: type[TField] | type[str] = str,
    ) -> TField | None:
        setting = self._try_get_setting(key)

        if not isinstance(setting, SettingLookup.Found):
            return None

        value = setting.value

        if value is None:
            return None

        raw_value: object = value
        typed_value_type = TypedType.from_type(value_type)

        if value == "" and typed_value_type.is_sequence:
            raw_value = []

        return cast("TField", TypeAdapter(value_type).validate_python(raw_value))

    @override
    def get_required_value[TField = str](
        self,
        key: str,
        value_type: type[TField] | type[str] = str,
    ) -> TField:
        """Get a setting value by its key or raise an error if the key is not found or the value is `None`. Optionally, validate the setting value against the specified type."""
        setting = self._try_get_setting(key)

        if not isinstance(setting, SettingLookup.Found):
            error_message = f"Missing setting value for key '{key}'"
            raise KeyError(error_message)

        value = setting.value

        if value is None:
            error_message = f"Setting value for key '{key}' is None"
            raise ValueError(error_message)

        raw_value: object = value
        typed_value_type = TypedType.from_type(value_type)

        if value == "" and typed_value_type.is_sequence:
            raw_value = []

        return cast("TField", TypeAdapter(value_type).validate_python(raw_value))

    @override
    def get_section(self, key: str) -> SettingsSection:
        """Get a settings section for the specified key. A settings section represents a subsection of the settings values that share a common key prefix."""
        if not self._is_section_key(key):
            error_message = f"Setting key '{key}' is not a section"
            raise KeyError(error_message)

        return SettingsSection(self, key)

    @override
    def get_children(self, key: str | None = None) -> list[SettingsSection]:
        path = key
        child_keys: list[str] = []

        for provider in reversed(self.providers):
            for item_key in provider.data:
                candidate = ""

                if path is None:
                    candidate = item_key
                else:
                    prefix = f"{path}{SettingsPath.KEY_DELIMITER}"

                    if not item_key.startswith(prefix):
                        continue

                    candidate = item_key[len(prefix) :]

                if len(candidate) == 0:
                    continue

                child_key = candidate.split(SettingsPath.KEY_DELIMITER, 1)[0]

                if len(child_key) == 0:
                    continue

                if child_key not in child_keys:
                    child_keys.append(child_key)

        children: list[SettingsSection] = []

        for child_key in child_keys:
            if path is None:
                child_path = child_key
            else:
                child_path = f"{path}{SettingsPath.KEY_DELIMITER}{child_key}"

            children.append(SettingsSection(self, child_path))

        return children

    def _is_section_key(self, key: str) -> bool:
        children = self.get_children(key)

        if len(children) == 0:
            return False

        return any(not child.key.isdigit() for child in children)
