from typing import Final, final, override

from wirio_settings.aws_secrets_manager.aws_secrets_manager_settings_provider import (
    AwsSecretsManagerSettingsProvider,
)
from wirio_settings.core.settings_builder import SettingsBuilder
from wirio_settings.core.settings_provider import SettingsProvider
from wirio_settings.core.settings_source import SettingsSource


@final
class AwsSecretsManagerSettingsSource(SettingsSource):
    _secret_id: Final[str]
    _region: Final[str | None]
    _url: Final[str | None]
    _access_key_id: Final[str | None]
    _secret_access_key: Final[str | None]
    _session_token: Final[str | None]
    _profile: Final[str | None]

    def __init__(  # noqa: PLR0913
        self,
        secret_id: str,
        region: str | None = None,
        url: str | None = None,
        access_key_id: str | None = None,
        secret_access_key: str | None = None,
        session_token: str | None = None,
        profile: str | None = None,
    ) -> None:
        self._secret_id = secret_id
        self._region = region
        self._url = url
        self._access_key_id = access_key_id
        self._secret_access_key = secret_access_key
        self._session_token = session_token
        self._profile = profile

    @override
    def build(self, builder: SettingsBuilder) -> SettingsProvider:
        return AwsSecretsManagerSettingsProvider(
            secret_id=self._secret_id,
            region=self._region,
            url=self._url,
            access_key_id=self._access_key_id,
            secret_access_key=self._secret_access_key,
            session_token=self._session_token,
            profile=self._profile,
        )
