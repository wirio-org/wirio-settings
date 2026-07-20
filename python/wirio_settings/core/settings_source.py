from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

from wirio_settings._wirio_settings import SettingsProvider

if TYPE_CHECKING:
    from wirio_settings.core.settings_builder import SettingsBuilder


class SettingsSource(ABC):
    """Source of setting values."""

    @abstractmethod
    def build(self, builder: "SettingsBuilder") -> SettingsProvider: ...
