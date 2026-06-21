from typing import Final, final, override

from google.auth.credentials import Credentials

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
    GcpSecretManagerSettingsProvider,
)


@final
class GcpSecretManagerSettingsSource(SettingsSource):
    _project_id: Final[str]
    _credentials: Final[Credentials | None]

    def __init__(
        self,
        project_id: str,
        credentials: Credentials | None = None,
    ) -> None:
        self._project_id = project_id
        self._credentials = credentials

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return GcpSecretManagerSettingsProvider(
            project_id=self._project_id, credentials=self._credentials
        )
