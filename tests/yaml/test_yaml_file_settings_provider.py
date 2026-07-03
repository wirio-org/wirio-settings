from decimal import Decimal
from pathlib import Path

from pydantic import BaseModel
from wirio_settings.settings_manager import SettingsManager


class TestYamlFileSettingsProvider:
    async def test_get_model(self, tmp_path: Path) -> None:
        class Submodel(BaseModel):
            subfield_1: str
            subfield_2: int

        class Settings(BaseModel):
            app_name: str
            port: int
            enabled: bool
            notes: str | None
            notes_2: str | None
            price_as_float: float
            price_as_decimal: Decimal
            int_list: list[int]
            string_list: list[str]
            submodel: Submodel

        expected_app_name = "wirio"
        expected_port = 8080
        expected_enabled = True
        expected_notes = None
        expected_price_as_float = 19.99
        expected_price_as_decimal = Decimal("19.99")
        expected_int_list = [1, 2, 3]
        expected_string_list = ["a", "b", "c"]
        expected_submodel = Submodel(subfield_1="value_1", subfield_2=42)
        file_path = tmp_path / "settings.yaml"
        file_path.write_text(
            """
appName: wirio
port: 8080
enabled: true
notes: null
notes_2:
priceAsFloat: 19.99
priceAsDecimal: 19.99
intList:
  - 1
  - 2
  - 3
stringList:
  - a
  - b
  - c
submodel:
  subfield_1: value_1
  subfield2: 42
""".strip(),
            encoding="utf-8",
        )

        settings_manager = SettingsManager(
            content_root_path=str(tmp_path), add_default_providers=False
        )
        settings_manager.add_yaml_file(
            path="settings.yaml",
            optional=False,
        )

        model = settings_manager.get_model(Settings)

        assert model.app_name == expected_app_name
        assert model.port == expected_port
        assert model.enabled == expected_enabled
        assert model.notes == expected_notes
        assert model.price_as_float == expected_price_as_float
        assert model.price_as_decimal == expected_price_as_decimal
        assert model.int_list == expected_int_list
        assert model.string_list == expected_string_list
        assert model.submodel.subfield_1 == expected_submodel.subfield_1
        assert model.submodel.subfield_2 == expected_submodel.subfield_2
