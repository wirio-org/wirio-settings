from .azure_identity import _AzureCredential
from .core import _ModelRegistry, _SettingLookup, _SettingsProvider, _SettingsSource
from collections.abc import Sequence
from typing import Final, final

@final
class _AzureAppConfigurationSettingsProvider(_SettingsProvider):
    def __str__(self, /) -> str: ...
    def data(self, /) -> "dict[str, str | None]": ...
    def load(self, /) -> None: ...
    def set_model_registry(self, /, model_registry: _ModelRegistry) -> None: ...
    def try_get(self, /, key: str) -> _SettingLookup: ...

@final
class _AzureAppConfigurationSettingsSource(_SettingsSource):
    def __new__(cls, /, endpoint: str, credential: _AzureCredential, selectors: Sequence[_SettingSelector] |None = None, trim_key_prefixes: Sequence[str] |None = None, feature_flag_selectors: Sequence[_FeatureFlagSelector] |None = None, feature_flag_trim_name_prefixes: Sequence[str] |None = None) -> _AzureAppConfigurationSettingsSource: ...
    def build(self, /) -> _SettingsProvider: ...

@final
class _FeatureFlagSelector:
    def __new__(cls, /, name_filter: str, label_filter: str |None = None) -> _FeatureFlagSelector:
        """
        Creates a feature flag selector. When `label_filter` is not specified, it loads values without a label.
        """

@final
class _KeyFilter:
    ANY: Final = "*"

@final
class _LabelFilter:
    NULL: Final = "\0"

@final
class _NameFilter:
    ANY: Final = "*"

@final
class _SettingSelector:
    def __new__(cls, /, key_filter: str |None = None, label_filter: str |None = None, snapshot_name: str |None = None) -> _SettingSelector:
        """
        Creates a setting selector. When `label_filter` is not specified, it loads values without a label. `snapshot_name` cannot be combined with filters.
        """
