from ..._wirio_settings import gcp_secret_manager

GcpSecretManagerSettingsProvider = gcp_secret_manager._GcpSecretManagerSettingsProvider
GcpSecretManagerSettingsSource = gcp_secret_manager._GcpSecretManagerSettingsSource

__all__ = [
    "GcpSecretManagerSettingsProvider",
    "GcpSecretManagerSettingsSource",
]
