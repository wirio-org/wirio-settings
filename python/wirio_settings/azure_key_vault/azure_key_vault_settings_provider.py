from typing import Final, final, override

from wirio_settings._wirio_settings import PythonAzureKeyVaultSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class AzureKeyVaultSettingsProvider(SettingsProvider):
    _url: Final[str]
    _client_id: Final[str | None]
    _client_secret: Final[str | None]
    _tenant_id: Final[str | None]

    def __init__(
        self,
        url: str,
        client_id: str | None = None,
        client_secret: str | None = None,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__()
        self._url = url
        self._client_id = client_id
        self._client_secret = client_secret
        self._tenant_id = tenant_id

    @override
    def load(self) -> None:
        provider = PythonAzureKeyVaultSettingsProvider(
            url=self._url,
            client_id=self._client_id,
            client_secret=self._client_secret,
            tenant_id=self._tenant_id,
        )
        provider.load()
        self._data = provider.data
