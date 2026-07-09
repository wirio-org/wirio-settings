from typing import Final, final, override

from wirio_settings._wirio_settings import PythonGcpSecretManagerSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class GcpSecretManagerSettingsProvider(SettingsProvider):
    _project_id: Final[str]
    _credentials_json: Final[str | None]

    def __init__(
        self,
        project_id: str,
        credentials_json: str | None = None,
    ) -> None:
        super().__init__()
        self._project_id = project_id
        self._credentials_json = credentials_json

    @override
    async def load(self) -> None:
        provider = PythonGcpSecretManagerSettingsProvider(
            project_id=self._project_id,
            credentials_json=self._credentials_json,
        )
        provider.load()
        self._data = provider.data
