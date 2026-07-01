from typing import Final, final, override

from wirio_settings._wirio_settings import PythonJsonFileSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class JsonFileSettingsProvider(SettingsProvider):
    _path: Final[str]
    _optional: Final[bool]

    def __init__(self, path: str, optional: bool) -> None:
        super().__init__()
        self._path = path
        self._optional = optional

    @override
    async def load(self) -> None:
        provider = PythonJsonFileSettingsProvider(
            path=self._path, optional=self._optional
        )
        await provider.load()
        self._data = provider.data
