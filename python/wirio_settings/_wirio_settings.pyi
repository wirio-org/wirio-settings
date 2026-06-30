from typing import Final, final

@final
class ConventionChanger:
    @staticmethod
    def to_snake_case(string_to_convert: str) -> str:
        """
        Convert a PascalCase, camelCase, or kebab-case string to snake_case.
        """

@final
class PythonEnvironmentVariablesSettingsProvider:
    def __str__(self, /) -> str: ...
    @staticmethod
    async def load() -> dict[str, str |None]: ...

@final
class PythonJsonFileSettingsProvider:
    def __str__(self, /) -> str: ...
    @staticmethod
    async def load() -> dict[str, str |None]: ...

@final
class SettingsPath:
    KEY_DELIMITER: Final = "."
    @staticmethod
    def get_section_key(path: str) -> str: ...
