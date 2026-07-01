from typing import Final, final

@final
class ConventionChanger:
    @staticmethod
    def to_snake_case(string_to_convert: str) -> str:
        """
        Convert a `PascalCase`, `camelCase`, or `kebab-case` string to `snake_case`.
        """

@final
class PythonEnvironmentVariablesSettingsProvider:
    def __new__(cls, /) -> PythonEnvironmentVariablesSettingsProvider: ...
    def __str__(self, /) -> str: ...
    @property
    def data(self, /) -> dict[str, str |None]: ...
    async def load(self, /) -> None: ...

@final
class PythonJsonSettingsProvider:
    def __new__(cls, /, path: str, optional: bool) -> PythonJsonSettingsProvider: ...
    def __str__(self, /) -> str: ...
    @property
    def data(self, /) -> dict[str, str |None]: ...
    async def load(self, /) -> None: ...

@final
class SettingsPath:
    KEY_DELIMITER: Final = "."
    @staticmethod
    def get_section_key(path: str) -> str: ...
