import os

import pytest
from wirio_settings.environment_variables.environment_variables_settings_provider import (
    EnvironmentVariablesSettingsProvider,
)


class TestEnvironmentVariablesSettingsProvider:
    async def test_load_environment_variables(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        expected_value = "WARNING"
        monkeypatch.setattr(
            os,
            "environ",
            {"LOGGING__LOG_LEVEL__DEFAULT": expected_value},
        )
        provider = EnvironmentVariablesSettingsProvider()

        await provider.load()

        assert provider.data == {"logging.log_level.default": expected_value}
