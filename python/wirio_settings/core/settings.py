from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

from pydantic import BaseModel

from wirio_settings.core.settings_binder import SettingsBinder

if TYPE_CHECKING:
    from wirio_settings.core.settings_section import SettingsSection


class Settings(ABC):
    """A level in the settings hierarchy."""

    @abstractmethod
    def get_value[TField](
        self,
        key: str,
        value_type: type[TField] | type[str] = str,
    ) -> TField | None: ...

    @abstractmethod
    def get_required_value[TField](
        self,
        key: str,
        value_type: type[TField] | type[str] = str,
    ) -> TField: ...

    @abstractmethod
    def get_section(self, key: str) -> "SettingsSection": ...

    @abstractmethod
    def get_children(
        self,
        key: str | None = None,
    ) -> list["SettingsSection"]: ...

    def get_model[TModel: BaseModel](self, model_type: type[TModel]) -> TModel:
        """Get a settings model of the specified type. The settings will be mapped to the model fields by their names."""
        return SettingsBinder.bind_model(self, model_type)
