import json
from typing import Any, Final, cast, final, override

import boto3

from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.json._json_settings_file_parser import JsonSettingsFileParser


@final
class AwsSecretsManagerSettingsProvider(SettingsProvider):
    _secret_id: Final[str]
    _region: Final[str | None]
    _url: Final[str | None]

    def __init__(
        self, secret_id: str, region: str | None = None, url: str | None = None
    ) -> None:
        super().__init__()
        self._secret_id = secret_id
        self._region = region
        self._url = url

    @override
    async def load(self) -> None:
        secrets_manager_client = cast(
            "Any",
            boto3.client(
                service_name="secretsmanager",
                region_name=self._region,
                endpoint_url=self._url,
            ),
        )
        secret = cast(
            "dict[str, Any]",
            secrets_manager_client.get_secret_value(SecretId=self._secret_id),
        )
        secret_value_str = cast("str", secret["SecretString"])
        secret_value_json = json.loads(secret_value_str)
        self._data = JsonSettingsFileParser().parse_json(
            cast("dict[str, Any]", secret_value_json)
        )
        await super().load()
