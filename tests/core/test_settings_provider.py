from wirio_settings._wirio_settings import (
    AwsSecretsManagerSettingsProvider,
    AzureKeyVaultSettingsProvider,
    EnvironmentVariablesSettingsProvider,
    GcpSecretManagerSettingsProvider,
    JsonFileSettingsProvider,
    KeyPerFileSettingsProvider,
    YamlFileSettingsProvider,
)


class TestSettingsProvider:
    def test_stringify_all_providers(self) -> None:
        assert (
            str(AzureKeyVaultSettingsProvider(url="https://settings.vault.azure.net"))
            == "AzureKeyVaultSettingsProvider"
        )
        assert (
            str(
                AwsSecretsManagerSettingsProvider(secret_id="settings")  # noqa: S106
            )
            == "AwsSecretsManagerSettingsProvider"
        )
        assert (
            str(GcpSecretManagerSettingsProvider(project_id="settings"))
            == "GcpSecretManagerSettingsProvider"
        )
        assert (
            str(EnvironmentVariablesSettingsProvider())
            == "EnvironmentVariablesSettingsProvider"
        )
        assert (
            str(
                YamlFileSettingsProvider(
                    content_root_path=None, path="settings.yaml", optional=True
                )
            )
            == "YamlFileSettingsProvider"
        )
        assert (
            str(
                JsonFileSettingsProvider(
                    content_root_path=None, path="settings.json", optional=True
                )
            )
            == "JsonFileSettingsProvider"
        )
        assert (
            str(KeyPerFileSettingsProvider(directory_path="settings", optional=True))
            == "KeyPerFileSettingsProvider"
        )
