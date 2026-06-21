from typing import ClassVar, final


@final
class ExtraDependencies:
    AZURE_KEY_VAULT_NOT_INSTALLED_ERROR_MESSAGE: ClassVar[str] = (
        "'azure-keyvault-secrets', 'azure-identity' or 'aiohttp' are not installed. Please, run 'uv add wirio-settings[azure-key-vault]' to install the required dependencies"
    )
    AWS_SECRETS_MANAGER_NOT_INSTALLED_ERROR_MESSAGE: ClassVar[str] = (
        "'boto3' is not installed. Please, run 'uv add wirio-settings[aws-secrets-manager]' to install the required dependencies"
    )
    GCP_SECRET_MANAGER_NOT_INSTALLED_ERROR_MESSAGE: ClassVar[str] = (
        "'google-cloud-secret-manager' is not installed. Please, run 'uv add wirio-settings[gcp-secret-manager]' to install the required dependencies"  # noqa: S105
    )

    @staticmethod
    def is_azure_key_vault_installed() -> bool:
        try:
            import aiohttp  # noqa: F401, PLC0415
            import azure.core.credentials  # noqa: PLC0415
            import azure.identity.aio  # noqa: F401, PLC0415
        except ImportError:
            return False

        return True

    @classmethod
    def ensure_azure_key_vault_is_installed(cls) -> None:
        try:
            import aiohttp  # noqa: F401, PLC0415
            import azure.core.credentials  # noqa: PLC0415
            import azure.identity.aio  # noqa: F401, PLC0415
        except ImportError as error:
            raise ImportError(
                cls.AZURE_KEY_VAULT_NOT_INSTALLED_ERROR_MESSAGE
            ) from error

    @staticmethod
    def is_aws_secrets_manager_installed() -> bool:
        try:
            import boto3  # noqa: F401, PLC0415
        except ImportError:
            return False

        return True

    @classmethod
    def ensure_aws_secrets_manager_is_installed(cls) -> None:
        try:
            import boto3  # noqa: F401, PLC0415
        except ImportError as error:
            raise ImportError(
                cls.AWS_SECRETS_MANAGER_NOT_INSTALLED_ERROR_MESSAGE
            ) from error

    @staticmethod
    def is_gcp_secret_manager_installed() -> bool:
        try:
            import google.cloud.secretmanager  # noqa: F401, PLC0415
        except ImportError:
            return False

        return True

    @classmethod
    def ensure_gcp_secret_manager_is_installed(cls) -> None:
        try:
            import google.cloud.secretmanager  # noqa: F401, PLC0415
        except ImportError as error:
            raise ImportError(
                cls.GCP_SECRET_MANAGER_NOT_INSTALLED_ERROR_MESSAGE
            ) from error
