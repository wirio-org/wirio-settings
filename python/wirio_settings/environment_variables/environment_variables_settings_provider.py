from typing import final, override

from wirio_settings._wirio_settings import PythonEnvironmentVariablesSettingsProvider
from wirio_settings.core.settings_provider import SettingsProvider


@final
class EnvironmentVariablesSettingsProvider(SettingsProvider):
    @override
    def load(self) -> None:
        provider = PythonEnvironmentVariablesSettingsProvider()
        provider.load()
        self._data = provider.data
