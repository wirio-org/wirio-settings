from typing import Final, final, override

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.json.json_file_settings_provider import JsonFileSettingsProvider


@final
class JsonSettingsSource(SettingsSource):
    _content_root_path: Final[str | None]
    _path: Final[str]
    _optional: Final[bool]

    def __init__(
        self, content_root_path: str | None, path: str, optional: bool
    ) -> None:
        self._content_root_path = content_root_path
        self._path = path
        self._optional = optional

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return JsonFileSettingsProvider(
            content_root_path=self._content_root_path,
            path=self._path,
            optional=self._optional,
        )
