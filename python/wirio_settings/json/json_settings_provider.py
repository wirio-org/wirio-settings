from pathlib import Path
from typing import Final, final, override

from wirio_settings._wirio_settings import PythonEnvironmentVariablesSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class JsonSettingsProvider(SettingsProvider):
    _path: Final[Path]
    _optional: Final[bool]

    def __init__(self, path: Path, optional: bool) -> None:
        super().__init__()
        self._path = path
        self._optional = optional

    @override
    async def load(self) -> None:
        provider = PythonEnvironmentVariablesSettingsProvider()
        await provider.load()
        self._data = provider.data
