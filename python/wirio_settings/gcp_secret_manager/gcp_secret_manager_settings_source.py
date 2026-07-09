from typing import Final, final, override

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.gcp_secret_manager.gcp_secret_manager_settings_provider import (
    GcpSecretManagerSettingsProvider,
)


@final
class GcpSecretManagerSettingsSource(SettingsSource):
    _project_id: Final[str]
    _credentials_json: Final[str | None]

    def __init__(
        self,
        project_id: str,
        credentials_json: str | None = None,
    ) -> None:
        self._project_id = project_id
        self._credentials_json = credentials_json

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return GcpSecretManagerSettingsProvider(
            project_id=self._project_id, credentials_json=self._credentials_json
        )
