from pathlib import Path
from typing import final, override

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.yaml.yaml_settings_provider import YamlSettingsProvider


@final
class YamlSettingsSource(SettingsSource):
    def __init__(self, path: Path, optional: bool) -> None:
        self._path = path
        self._optional = optional

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return YamlSettingsProvider(path=self._path, optional=self._optional)
