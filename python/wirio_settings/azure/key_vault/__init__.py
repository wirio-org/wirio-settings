from ..._wirio_settings import azure_key_vault

AzureKeyVaultSettingsProvider = azure_key_vault._AzureKeyVaultSettingsProvider
AzureKeyVaultSettingsSource = azure_key_vault._AzureKeyVaultSettingsSource

__all__ = [
    "AzureKeyVaultSettingsProvider",
    "AzureKeyVaultSettingsSource",
]
