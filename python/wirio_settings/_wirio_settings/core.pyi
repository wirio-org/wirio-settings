from typing import Any, Final, final
from weakref import ReferenceType

@final
class _ModelRegistry:
    def __new__(cls, /, refresh_models_callback: ReferenceType) -> _ModelRegistry: ...
    def add_model(self, /, model: Any, section_path: str |None = None) -> None:
        """
        Adds a Pydantic model to the registry.
        """
    def models(self, /) -> "list[_RegisteredModel]":
        """
        Returns all tracked Pydantic models. It will automatically remove models that have been garbage collected.
        """

@final
class _RegisteredModel:
    @property
    def model_reference(self, /) -> ReferenceType: ...
    @property
    def section_path(self, /) -> str |None: ...

class _SettingLookup:
    @final
    class Found(_SettingLookup):
        __match_args__: Final = ("value",)
        def __new__(cls, /, value: str |None) -> _SettingLookup.Found: ...
        @property
        def value(self, /) -> str |None: ...
    @final
    class Missing(_SettingLookup):
        __match_args__: Final = ()
        def __getitem__(self, key: int, /) -> Any: ...
        def __len__(self, /) -> int: ...
        def __new__(cls, /) -> _SettingLookup.Missing: ...

@final
class _SettingsPath:
    KEY_DELIMITER: Final = "."
    @staticmethod
    def get_section_key(path: str) -> str:
        """
        Returns the last section key from a given settings path.
        If the path does not contain the delimiter, it returns the original path.
        """

class _SettingsProvider:
    """
    Provides setting values.
    """
    def __new__(cls, /) -> _SettingsProvider: ...
    def data(self, /) -> "dict[str, str | None]": ...
    def load(self, /) -> None: ...
    def set_model_registry(self, /, model_registry: _ModelRegistry) -> None: ...
    def try_get(self, /, key: str) -> _SettingLookup: ...

class _SettingsSource:
    """
    Source of setting values.
    """
    def __new__(cls, /) -> _SettingsSource: ...
    def build(self, /) -> _SettingsProvider: ...
