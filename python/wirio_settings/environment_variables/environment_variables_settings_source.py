from typing import final, override

from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource
from wirio_settings.environment_variables.environment_variables_settings_provider import (
    EnvironmentVariablesSettingsProvider,
)


@final
class EnvironmentVariablesSettingsSource(SettingsSource):
    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return EnvironmentVariablesSettingsProvider()
