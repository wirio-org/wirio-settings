from ..._wirio_settings import aws_secrets_manager

AwsSecretsManagerSettingsProvider = (
    aws_secrets_manager._AwsSecretsManagerSettingsProvider
)
AwsSecretsManagerSettingsSource = aws_secrets_manager._AwsSecretsManagerSettingsSource

__all__ = [
    "AwsSecretsManagerSettingsProvider",
    "AwsSecretsManagerSettingsSource",
]
