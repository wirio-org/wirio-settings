from abc import ABC, abstractmethod
from typing import override

from wirio_settings.core.wirio_undefined import WirioUndefined


class SettingsProvider(ABC):
    """Represent a source of key/value settings for an application."""

    _data: dict[str, str | None]

    def __init__(self) -> None:
        self._data = {}

    @property
    def data(self) -> dict[str, str | None]:
        return self._data

    @abstractmethod
    def load(self) -> None: ...

    def try_get(self, key: str) -> str | None | WirioUndefined:
        return self._data.get(key, WirioUndefined.INSTANCE)

    @override
    def __str__(self) -> str:
        return self.__class__.__name__
