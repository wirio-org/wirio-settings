import os

from pytest_mock import MockerFixture
from wirio_settings.environment_variables.environment_variables_settings_provider import (
    EnvironmentVariablesSettingsProvider,
)


class TestEnvironmentVariablesSettingsProvider:
    async def test_load_environment_variables(self, mocker: MockerFixture) -> None:
        expected_value = "WARNING"
        mocker.patch.dict(
            os.environ, {"LOGGING__LOG_LEVEL__DEFAULT": expected_value}, clear=True
        )
        provider = EnvironmentVariablesSettingsProvider()

        await provider.load()

        assert provider.data == {"logging.log_level.default": expected_value}
