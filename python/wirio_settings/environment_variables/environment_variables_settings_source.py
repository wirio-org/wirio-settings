from typing import final, override

from wirio_settings._wirio_settings import (
    EnvironmentVariablesSettingsProvider,
    SettingsProvider,
)
from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_source import SettingsSource


@final
class EnvironmentVariablesSettingsSource(SettingsSource):
    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return EnvironmentVariablesSettingsProvider()
