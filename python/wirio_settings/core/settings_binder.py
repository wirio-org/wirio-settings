import typing
from types import NoneType, UnionType
from typing import (
    TYPE_CHECKING,
    Any,
    Union,
    cast,
)

from pydantic import BaseModel, TypeAdapter
from pydantic.fields import FieldInfo

from wirio_settings._wirio_settings import SettingLookup, SettingsPath
from wirio_settings.core._typed_type import TypedType

if TYPE_CHECKING:
    from wirio_settings.core.settings import Settings


class SettingsBinder:
    @classmethod
    def bind_model[TModel: BaseModel](
        cls,
        settings: "Settings",
        model_type: type[TModel],
    ) -> TModel:
        """Binds setting values to Pydantic models."""
        return cls._bind_instance(
            model_type=model_type,
            settings=settings,
            key_prefix="",
        )

    @classmethod
    def _bind_instance[TModel: BaseModel](
        cls,
        model_type: type[TModel],
        settings: "Settings",
        key_prefix: str,
    ) -> TModel:
        values: dict[str, object] = {}

        for field_name, field_info in model_type.model_fields.items():
            key = cls._join_key(key_prefix, field_name)
            value = cls._read_field_value(
                settings=settings,
                key=key,
                field_info=field_info,
            )

            if value is None:
                has_default_value = not field_info.is_required()
                if has_default_value:
                    values[field_name] = cls._get_field_default(field_info)
                    continue

                can_be_none = (
                    field_info.annotation is not None
                    and NoneType in typing.get_args(field_info.annotation)
                )

                if can_be_none:
                    values[field_name] = None
                    continue

                error_message = f"Missing setting value for key '{key}'"
                raise KeyError(error_message)

            values[field_name] = value

        return model_type.model_validate(values)

    @classmethod
    def _read_field_value(
        cls,
        settings: "Settings",
        key: str,
        field_info: FieldInfo,
    ) -> object | None:
        field_annotation = cls._extract_field_annotation(field_info.annotation)
        field_type = TypedType.from_type(field_annotation)

        if issubclass(field_type.to_type(), BaseModel):
            try:
                section = settings.get_section(key)
            except KeyError:
                return None

            return cls._bind_instance(
                model_type=field_type.annotation,
                settings=section,
                key_prefix="",
            )

        if field_type.is_sequence:
            collection_value = cls._read_collection_value(
                settings=settings,
                key=key,
                field_type=field_type,
            )

            if not isinstance(collection_value, SettingLookup.Missing):
                return collection_value

        if field_type.is_mapping:
            mapping_value = cls._read_mapping_value(
                settings=settings,
                key=key,
                field_type=field_type,
            )

            if not isinstance(mapping_value, SettingLookup.Missing):
                return mapping_value

        typed_value = settings.get_value(key, field_type.annotation)

        if typed_value is None:
            return None

        return cast(
            "object | None",
            TypeAdapter(field_type.annotation).validate_python(typed_value),
        )

    @classmethod
    def _read_collection_value(
        cls, settings: "Settings", key: str, field_type: TypedType
    ) -> object | SettingLookup.Missing:
        value_type = field_type.args[0]
        values: list[object] = []
        index = 0

        while True:
            indexed_key = cls._join_key(key, str(index))

            if issubclass(value_type, BaseModel):
                nested_values = cls._try_collect_model_values(
                    settings=settings,
                    model_type=value_type,
                    key_prefix=indexed_key,
                )

                if isinstance(nested_values, SettingLookup.Missing):
                    break

                values.append(value_type.model_validate(nested_values))
            else:
                raw_value: object | SettingLookup.Missing = cls._try_get_setting_value(
                    settings, indexed_key
                )

                if isinstance(raw_value, SettingLookup.Missing):
                    break

                typed_value: object = TypeAdapter(value_type).validate_python(raw_value)
                values.append(typed_value)

            index += 1

        if len(values) == 0:
            return SettingLookup.Missing()

        return TypeAdapter(field_type.annotation).validate_python(values)

    @classmethod
    def _read_mapping_value(
        cls,
        settings: "Settings",
        key: str,
        field_type: TypedType,
    ) -> object | SettingLookup.Missing:
        key_type = field_type.args[0]
        value_type = field_type.args[1]
        values: dict[object, object | None] = {}

        try:
            section = settings.get_section(key)
        except KeyError:
            return SettingLookup.Missing()

        children = section.get_children()

        for child_section in children:
            typed_key: object = TypeAdapter(key_type).validate_python(child_section.key)

            if issubclass(value_type, BaseModel):
                typed_value = cls._bind_instance(
                    model_type=value_type,
                    settings=child_section,
                    key_prefix="",
                )
                values[typed_key] = typed_value
                continue

            value_key = cls._join_key(key, child_section.key)
            raw_value: object | SettingLookup.Missing = cls._try_get_setting_value(
                settings, value_key
            )

            if isinstance(raw_value, SettingLookup.Missing):
                continue

            typed_value = TypeAdapter(value_type).validate_python(raw_value)
            values[typed_key] = typed_value

        if len(values) == 0:
            return SettingLookup.Missing()

        return TypeAdapter(field_type.annotation).validate_python(values)

    @classmethod
    def _try_collect_model_values(
        cls,
        settings: "Settings",
        model_type: type[BaseModel],
        key_prefix: str,
    ) -> object | SettingLookup.Missing:
        values: dict[str, object] = {}
        has_any_value = False

        for field_name, field_info in model_type.model_fields.items():
            key = cls._join_key(key_prefix, field_name)
            value = cls._read_field_value(
                settings=settings,
                key=key,
                field_info=field_info,
            )

            if value is None:
                if field_info.is_required():
                    continue

                values[field_name] = cls._get_field_default(field_info)
                continue

            has_any_value = True
            values[field_name] = value

        if not has_any_value:
            return SettingLookup.Missing()

        return values

    @classmethod
    def _try_get_setting_value(
        cls,
        settings: "Settings",
        key: str,
        value_type: type | None = None,
    ) -> object | SettingLookup.Missing:
        raw_value: object | None

        if value_type is None:
            raw_value = settings.get_value(key)
        else:
            raw_value = settings.get_value(key, value_type)

        if raw_value is None:
            return SettingLookup.Missing()

        return raw_value

    @classmethod
    def _join_key(cls, parent: str, child: str) -> str:
        if len(parent) == 0:
            return child

        if len(child) == 0:
            return parent

        return f"{parent}{SettingsPath.KEY_DELIMITER}{child}"

    @classmethod
    def _extract_field_annotation(
        cls,
        field_annotation: Any,  # noqa: ANN401
    ) -> Any:  # noqa: ANN401
        assert field_annotation is not None
        origin = typing.get_origin(field_annotation)

        if origin not in [UnionType, Union]:
            return field_annotation

        args = [
            value
            for value in typing.get_args(field_annotation)
            if value is not NoneType
        ]

        if len(args) == 1:
            return args[0]

        error_message = "A field type annotation cannot be resolved because it is a union of multiple types. Only unions of a single type with None are supported to indicate optional fields"
        raise RuntimeError(error_message)

    @classmethod
    def _get_field_default(cls, field_info: FieldInfo) -> object:
        return field_info.get_default(call_default_factory=True)
