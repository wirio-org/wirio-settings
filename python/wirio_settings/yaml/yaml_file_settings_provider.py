from typing import Final, final, override

from wirio_settings._wirio_settings import PythonYamlFileSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class YamlFileSettingsProvider(SettingsProvider):
    _content_root_path: Final[str | None]
    _path: Final[str]
    _optional: Final[bool]

    def __init__(
        self, content_root_path: str | None, path: str, optional: bool
    ) -> None:
        super().__init__()
        self._content_root_path = content_root_path
        self._path = path
        self._optional = optional

    @override
    async def load(self) -> None:
        provider = PythonYamlFileSettingsProvider(
            content_root_path=self._content_root_path,
            path=self._path,
            optional=self._optional,
        )
        await provider.load()
        self._data = provider.data
