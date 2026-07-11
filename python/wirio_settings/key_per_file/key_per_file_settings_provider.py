from typing import Final, final, override

from wirio_settings._wirio_settings import PythonKeyPerFileSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class KeyPerFileSettingsProvider(SettingsProvider):
    _directory_path: Final[str]
    _optional: Final[bool]

    def __init__(self, directory_path: str, optional: bool) -> None:
        super().__init__()
        self._directory_path = directory_path
        self._optional = optional

    @override
    async def load(self) -> None:
        provider = PythonKeyPerFileSettingsProvider(
            directory_path=self._directory_path,
            optional=self._optional,
        )
        provider.load()
        self._data = provider.data
