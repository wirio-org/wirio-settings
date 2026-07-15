import os
from pathlib import Path

import pytest
from pytest_mock import MockerFixture
from wirio_settings import SettingsManager


class TestIntegration:
    @pytest.mark.skipif(
        os.environ.get("INTEGRATION_TEST") is None, reason="Integration test"
    )
    def test_load_secrets_using_aws_secrets_manager(self) -> None:
        secret_id = "dev/test-secret-id"  # noqa: S105
        expected_secret_1 = "secret-value-1"  # noqa: S105
        expected_secret_2 = "secret-value-2"  # noqa: S105
        expected_nested_secret = "Nested-value"  # noqa: S105
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_aws_secrets_manager(
            secret_id=secret_id,
        )

        assert settings_manager.get_required_value("secret_1") == expected_secret_1
        assert settings_manager.get_required_value("secret_2") == expected_secret_2
        assert (
            settings_manager.get_required_value("parent.nested_secret")
            == expected_nested_secret
        )

    @pytest.mark.skipif(
        os.environ.get("INTEGRATION_TEST") is None, reason="Integration test"
    )
    def test_load_secrets_using_gcp_secret_manager(self) -> None:
        project_id = os.environ["GCP_PROJECT_ID"]
        expected_secret_1 = "secret-value-1"  # noqa: S105
        expected_secret_2 = "secret-value-2"  # noqa: S105
        expected_nested_secret = "Nested-value"  # noqa: S105
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_gcp_secret_manager(
            project_id=project_id,
        )

        assert settings_manager.get_required_value("secret_1") == expected_secret_1
        assert settings_manager.get_required_value("secret_2") == expected_secret_2
        assert (
            settings_manager.get_required_value("parent.nested_secret")
            == expected_nested_secret
        )

    @pytest.mark.skipif(
        os.environ.get("INTEGRATION_TEST") is None, reason="Integration test"
    )
    def test_load_secrets_using_azure_key_vault(self) -> None:
        key_vault_url = "https://kv-wiriosettings-001.vault.azure.net"
        expected_secret_1 = "secret-value-1"  # noqa: S105
        expected_secret_2 = "secret-value-2"  # noqa: S105
        expected_nested_secret = "Nested-value"  # noqa: S105
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_azure_key_vault(url=key_vault_url)

        assert settings_manager.get_required_value("secret_1") == expected_secret_1
        assert settings_manager.get_required_value("secret_2") == expected_secret_2
        assert (
            settings_manager.get_required_value("parent.nested_secret")
            == expected_nested_secret
        )

    @pytest.mark.skipif(
        os.environ.get("INTEGRATION_TEST") is None, reason="Integration test"
    )
    def test_load_settings_using_environment_variables(self) -> None:
        expected_feature_flag = "true"
        expected_nested_value = "nested-value"
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_environment_variables()

        assert (
            settings_manager.get_required_value("test_integration_feature_flag_enabled")
            == expected_feature_flag
        )
        assert (
            settings_manager.get_required_value("test_integration_parent.nested_value")
            == expected_nested_value
        )

    def test_load_settings_using_key_per_file_directory(self, tmp_path: Path) -> None:
        expected_app_name = "wirio"
        expected_log_level = "WARNING"
        expected_password = "secret-value"  # noqa: S105
        tmp_path.joinpath("app_name").write_text(expected_app_name)
        tmp_path.joinpath("logging__log_level__default").write_text(expected_log_level)
        tmp_path.joinpath("database_password").write_text(f"{expected_password}\n")
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_key_per_file(directory_path=str(tmp_path))

        assert settings_manager.get_required_value("app_name") == expected_app_name
        assert (
            settings_manager.get_required_value("logging__log_level__default")
            == expected_log_level
        )
        assert (
            settings_manager.get_required_value("database_password")
            == expected_password
        )

    def test_load_settings_using_yaml_file(self, tmp_path: Path) -> None:
        expected_app_name = "wirio"
        expected_port = "8080"
        expected_log_level = "warning"
        settings_file_path = tmp_path.joinpath("settings.yaml")
        settings_file_path.write_text(
            """
appName: wirio
port: 8080
logging:
  logLevel:
    default: warning
""".strip()
        )
        settings_manager = SettingsManager(
            content_root_path="",
            add_default_providers=False,
        )

        settings_manager.add_yaml_file(path=str(settings_file_path))

        assert settings_manager.get_required_value("app_name") == expected_app_name
        assert settings_manager.get_required_value("port") == expected_port
        assert (
            settings_manager.get_required_value("logging.log_level.default")
            == expected_log_level
        )

    def test_load_settings_using_default_providers_without_already_running_event_loop(
        self, tmp_path: Path, mocker: MockerFixture
    ) -> None:
        self._shared_test_load_settings_using_default_providers(tmp_path, mocker)

    async def test_load_settings_using_default_providers_from_running_event_loop(
        self, tmp_path: Path, mocker: MockerFixture
    ) -> None:
        self._shared_test_load_settings_using_default_providers(tmp_path, mocker)

    def _shared_test_load_settings_using_default_providers(
        self, tmp_path: Path, mocker: MockerFixture
    ) -> None:
        expected_from_yaml_settings = "YAML settings"
        expected_from_integration_yaml_settings = "Integration YAML settings"
        expected_from_environment_variables = "Environment variables"
        shared_setting = "Shared setting"

        tmp_path.joinpath("settings.yaml").write_text(
            """
FromYamlSettings: YAML settings
sharedSetting: YAML settings
""".strip()
        )
        tmp_path.joinpath("settings.integration.yaml").write_text(
            """
fromIntegrationYamlSettings: Integration YAML settings
sharedSetting: Integration YAML settings
""".strip()
        )

        mocker.patch.dict(
            os.environ,
            {
                "WIRIO_ENVIRONMENT": "integration",
                "FROM_ENVIRONMENT": expected_from_environment_variables,
                "SHARED_SETTING": shared_setting,
            },
        )

        settings_manager = SettingsManager(content_root_path=str(tmp_path))

        assert (
            settings_manager.get_required_value("from_yaml_settings")
            == expected_from_yaml_settings
        )
        assert (
            settings_manager.get_required_value("from_integration_yaml_settings")
            == expected_from_integration_yaml_settings
        )
        assert (
            settings_manager.get_required_value("from_environment")
            == expected_from_environment_variables
        )
        assert settings_manager.get_required_value("shared_setting") == shared_setting
