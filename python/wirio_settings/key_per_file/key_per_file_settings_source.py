from typing import Final, final, override

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.key_per_file.key_per_file_settings_provider import (
    KeyPerFileSettingsProvider,
)


@final
class KeyPerFileSettingsSource(SettingsSource):
    _directory_path: Final[str]
    _optional: Final[bool]

    def __init__(self, directory_path: str, optional: bool) -> None:
        self._directory_path = directory_path
        self._optional = optional

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return KeyPerFileSettingsProvider(
            directory_path=self._directory_path,
            optional=self._optional,
        )
