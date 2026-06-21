from typing import Final, final, override

from google.auth.credentials import Credentials
from google.cloud.secretmanager import (
    AccessSecretVersionRequest,
    ListSecretsRequest,
    SecretManagerServiceAsyncClient,
)

from wirio_settings.core.settings_path import SettingsPath
from wirio_settings.core.settings_provider import SettingsProvider


@final
class GcpSecretManagerSettingsProvider(SettingsProvider):
    _project_id: Final[str]
    _credentials: Final[Credentials | None]

    def __init__(
        self,
        project_id: str,
        credentials: Credentials | None = None,
    ) -> None:
        super().__init__()
        self._project_id = project_id
        self._credentials = credentials

    @override
    async def load(self) -> None:
        async with SecretManagerServiceAsyncClient(
            credentials=self._credentials
        ) as secret_client:
            secret_names = await self._get_secret_names(secret_client)

            for secret_name in secret_names:
                secret_version_path = secret_client.secret_version_path(
                    self._project_id, secret_name, "latest"
                )
                access_secret_version_request = AccessSecretVersionRequest(
                    name=secret_version_path
                )
                access_secret_version_response = (
                    await secret_client.access_secret_version(
                        access_secret_version_request
                    )
                )
                secret_value = access_secret_version_response.payload.data.decode(
                    "utf-8"
                )
                normalized_secret_name = self._normalize_key(secret_name)
                self._data[normalized_secret_name] = secret_value

        await super().load()

    async def _get_secret_names(
        self, secret_client: SecretManagerServiceAsyncClient
    ) -> list[str]:
        list_secrets_request = ListSecretsRequest(parent=f"projects/{self._project_id}")
        list_secrets_response = await secret_client.list_secrets(
            request=list_secrets_request
        )
        secret_ids = [secret.name async for secret in list_secrets_response]
        return [secret_id.split("/")[-1] for secret_id in secret_ids]

    def _normalize_key(self, key: str) -> str:
        return key.replace("--", SettingsPath.KEY_DELIMITER)
