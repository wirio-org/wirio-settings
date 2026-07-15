from typing import final, override

from wirio_settings.core.settings_provider import SettingsProvider


class TestSettingsProvider:
    def test_stringify(self) -> None:
        @final
        class SettingsProviderFake(SettingsProvider):
            @override
            def load(self) -> None:
                pass

        provider = SettingsProviderFake()

        assert str(provider) == SettingsProviderFake.__name__
