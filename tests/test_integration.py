import os

import pytest
from wirio_settings import SettingsManager


@pytest.mark.skipif(
    os.environ.get("INTEGRATION_TEST") is None, reason="Integration tests"
)
class TestIntegration:
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

    def test_load_secrets_using_azure_key_vault(self) -> None:
        key_vault_url = os.environ["AZURE_KEY_VAULT_URL"]
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
