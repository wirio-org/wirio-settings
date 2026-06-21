from collections.abc import AsyncIterator
from typing import TYPE_CHECKING, Any, cast

import pytest
from pytest_mock import MockerFixture
from wirio_settings.core._extra_dependencies import ExtraDependencies

if TYPE_CHECKING:
    from google.auth.credentials import Credentials
    from google.cloud.secretmanager import (
        AccessSecretVersionResponse,
        ListSecretsResponse,
        Secret,
        SecretManagerServiceAsyncClient,
    )
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
        GcpSecretManagerSettingsProvider,
    )
else:
    Credentials = Any
    AccessSecretVersionResponse = Any
    ListSecretsResponse = Any
    Secret = Any
    SecretManagerServiceAsyncClient = Any
    GcpSecretManagerSettingsProvider = Any

try:
    from google.auth.credentials import Credentials
    from google.cloud.secretmanager import (
        AccessSecretVersionResponse,
        ListSecretsResponse,
        Secret,
        SecretManagerServiceAsyncClient,
    )
    from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
        GcpSecretManagerSettingsProvider,
    )
except ImportError:
    pass


class _AccessSecretVersionResponsePayload:
    data: bytes


@pytest.mark.skipif(
    not ExtraDependencies.is_gcp_secret_manager_installed(),
    reason=ExtraDependencies.GCP_SECRET_MANAGER_NOT_INSTALLED_ERROR_MESSAGE,
)
class TestGcpSecretManagerSettingsProvider:
    @pytest.fixture(autouse=True)
    def setup(self) -> None:
        self.project_id = "my-project-id"

    async def test_load_secrets(
        self,
        mocker: MockerFixture,
    ) -> None:
        expected_secret_count = 2
        credentials_mock = mocker.create_autospec(Credentials, instance=True)

        first_secret_name = "ServiceApiKey"  # noqa: S105
        first_secret_value = "12345"  # noqa: S105
        second_secret_name = "Logging--LogLevel--Default"  # noqa: S105
        second_secret_value = "WARNING"  # noqa: S105

        first_secret_mock = mocker.create_autospec(Secret, instance=True)
        first_secret_mock.name = (
            f"projects/{self.project_id}/secrets/{first_secret_name}"
        )
        second_secret_mock = mocker.create_autospec(Secret, instance=True)
        second_secret_mock.name = (
            f"projects/{self.project_id}/secrets/{second_secret_name}"
        )

        async def list_secrets_response() -> AsyncIterator[Secret]:
            yield first_secret_mock
            yield second_secret_mock

        first_payload_mock = mocker.create_autospec(
            _AccessSecretVersionResponsePayload,
            instance=True,
        )
        first_payload_mock.data = first_secret_value.encode("utf-8")

        first_access_secret_version_response_mock = mocker.create_autospec(
            AccessSecretVersionResponse,
            instance=True,
        )
        first_access_secret_version_response_mock.payload = first_payload_mock

        second_payload_mock = mocker.create_autospec(
            _AccessSecretVersionResponsePayload,
            instance=True,
        )
        second_payload_mock.data = second_secret_value.encode("utf-8")

        second_access_secret_version_response_mock = mocker.create_autospec(
            AccessSecretVersionResponse,
            instance=True,
        )
        second_access_secret_version_response_mock.payload = second_payload_mock

        secret_client_mock = mocker.create_autospec(
            SecretManagerServiceAsyncClient,
            instance=True,
        )
        secret_client_mock.__aenter__.return_value = secret_client_mock
        secret_client_mock.__aexit__.return_value = None
        secret_client_mock.list_secrets.return_value = cast(
            "ListSecretsResponse", list_secrets_response()
        )
        secret_client_mock.secret_version_path.side_effect = [
            f"projects/{self.project_id}/secrets/{first_secret_name}/versions/latest",
            f"projects/{self.project_id}/secrets/{second_secret_name}/versions/latest",
        ]
        secret_client_mock.access_secret_version.side_effect = [
            first_access_secret_version_response_mock,
            second_access_secret_version_response_mock,
        ]

        secret_client_patch = mocker.patch(
            f"{GcpSecretManagerSettingsProvider.__module__}.{SecretManagerServiceAsyncClient.__qualname__}",
            autospec=True,
            return_value=secret_client_mock,
        )
        provider = GcpSecretManagerSettingsProvider(
            project_id=self.project_id,
            credentials=credentials_mock,
        )

        await provider.load()

        assert provider.data == {
            "service_api_key": first_secret_value,
            "logging.log_level.default": second_secret_value,
        }
        secret_client_patch.assert_called_once_with(credentials=credentials_mock)
        secret_client_mock.list_secrets.assert_called_once()
        assert (
            secret_client_mock.access_secret_version.call_count == expected_secret_count
        )
