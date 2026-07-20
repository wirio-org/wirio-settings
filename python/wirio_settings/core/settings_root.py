from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

from wirio_settings._wirio_settings import SettingLookup
from wirio_settings.core.settings import Settings

if TYPE_CHECKING:
    from wirio_settings._wirio_settings import SettingsProvider
    from wirio_settings.core.settings_section import SettingsSection


class SettingsRoot(Settings, ABC):
    """Represent the root node for settings."""

    @property
    @abstractmethod
    def providers(self) -> list["SettingsProvider"]: ...

    @abstractmethod
    def get_section(self, key: str) -> "SettingsSection": ...

    @abstractmethod
    def get_value[TField](
        self,
        key: str,
        value_type: type[TField] | type[str] = str,
    ) -> TField | None: ...

    def _try_get_setting(self, key: str) -> SettingLookup:
        for provider in reversed(self.providers):
            setting = provider.try_get(key)

            if isinstance(setting, SettingLookup.Found):
                return setting

        return SettingLookup.Missing()
