from abc import ABC
from typing import override

from wirio_settings.core.convention_changer import ConventionChanger
from wirio_settings.core.wirio_undefined import WirioUndefined


class SettingsProvider(ABC):
    """Represent a source of key/value settings for an application."""

    _data: dict[str, str | None]

    def __init__(self) -> None:
        self._data = {}

    @property
    def data(self) -> dict[str, str | None]:
        return self._data

    async def load(self) -> None:
        normalized_data: dict[str, str | None] = {}

        for item_key, item_data in self._data.items():
            item_key_in_snake_case = ConventionChanger.to_snake_case(item_key)
            normalized_data[item_key_in_snake_case] = item_data

        self._data = normalized_data

    def try_get(self, key: str) -> str | None | WirioUndefined:
        return self._data.get(key, WirioUndefined.INSTANCE)

    @override
    def __str__(self) -> str:
        return self.__class__.__name__
