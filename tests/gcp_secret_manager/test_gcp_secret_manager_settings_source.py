from typing import TYPE_CHECKING, Any

import pytest
from pytest_mock import MockerFixture
from wirio_settings.core._extra_dependencies import ExtraDependencies
from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider

if TYPE_CHECKING:
    from google.auth.credentials import Credentials
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
        GcpSecretManagerSettingsProvider,
    )
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_source import (
        GcpSecretManagerSettingsSource,
    )
else:
    Credentials = Any
    GcpSecretManagerSettingsProvider = Any
    GcpSecretManagerSettingsSource = Any

try:
    from google.auth.credentials import Credentials
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
        GcpSecretManagerSettingsProvider,
    )
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_source import (
        GcpSecretManagerSettingsSource,
    )
except ImportError:
    pass


@pytest.mark.skipif(
    not ExtraDependencies.is_gcp_secret_manager_installed(),
    reason=ExtraDependencies.GCP_SECRET_MANAGER_NOT_INSTALLED_ERROR_MESSAGE,
)
class TestGcpSecretManagerSettingsSource:
    def test_build_provider(self, mocker: MockerFixture) -> None:
        project_id = "project-id"
        credentials_mock = mocker.create_autospec(Credentials, instance=True)
        settings_provider_mock = mocker.create_autospec(SettingsProvider, instance=True)
        settings_builder_mock = mocker.create_autospec(SettingsBuilder, instance=True)
        settings_provider_patch = mocker.patch(
            f"{GcpSecretManagerSettingsSource.__module__}.{GcpSecretManagerSettingsProvider.__qualname__}",
            autospec=True,
            return_value=settings_provider_mock,
        )
        source = GcpSecretManagerSettingsSource(
            project_id=project_id,
            credentials=credentials_mock,
        )

        provider = source.build(settings_builder_mock)

        assert provider is settings_provider_mock
        settings_provider_patch.assert_called_once_with(
            project_id=project_id,
            credentials=credentials_mock,
        )
