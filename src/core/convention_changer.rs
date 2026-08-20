use regex::regex;

/// Convert a `PascalCase`, `camelCase`, or `kebab-case` string to `snake_case`.
pub fn to_snake_case(string_to_convert: &str) -> String {
    let regex_1 = regex!(r"([A-Z]+)([A-Z][a-z])");
    let regex_2 = regex!(r"([a-z])([A-Z])");
    let regex_3 = regex!(r"([0-9])([A-Z])");
    let regex_4 = regex!(r"([a-z])([0-9])");

    // Handle the sequence of uppercase letters followed by a lowercase letter
    let converted = regex_1.replace_all(string_to_convert, "${1}_${2}");

    // Insert an underscore between a lowercase letter and an uppercase letter
    let converted = regex_2.replace_all(&converted, "${1}_${2}");

    // Insert an underscore between a digit and an uppercase letter
    let converted = regex_3.replace_all(&converted, "${1}_${2}");

    // Insert an underscore between a lowercase letter and a digit
    let converted = regex_4.replace_all(&converted, "${1}_${2}");

    // Replace hyphens with underscores to handle kebab-case
    converted.replace('-', "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use crate::core::convention_changer;

    #[test]
    fn test_convert_to_snake_case() {
        let test_cases = vec![
            ("snake_to_snake", "snake_to_snake"),
            ("camelToSnake", "camel_to_snake"),
            ("camel2Snake", "camel_2_snake"),
            ("Camel2Snake", "camel_2_snake"),
            ("camel2snake", "camel_2snake"),
            ("_camelToSnake", "_camel_to_snake"),
            ("camelToSnake_", "camel_to_snake_"),
            ("__camelToSnake__", "__camel_to_snake__"),
            ("CamelToSnake", "camel_to_snake"),
            ("_CamelToSnake", "_camel_to_snake"),
            ("CamelToSnake_", "camel_to_snake_"),
            ("CAMELToSnake", "camel_to_snake"),
            ("__CamelToSnake__", "__camel_to_snake__"),
            ("Camel2", "camel_2"),
            ("Camel2_", "camel_2_"),
            ("_Camel2", "_camel_2"),
            ("camel2", "camel_2"),
            ("camel2_", "camel_2_"),
            ("_camel2", "_camel_2"),
            ("kebab-to-snake", "kebab_to_snake"),
            ("kebab-Snake", "kebab_snake"),
            ("Kebab-Snake", "kebab_snake"),
            ("PascalToSnake", "pascal_to_snake"),
            ("snakeV2", "snake_v2"),
            ("snakeVV2", "snake_vv2"),
            ("snakev2", "snakev_2"),
        ];

        for (string_to_convert, expected_string) in test_cases {
            let converted_value = convention_changer::to_snake_case(string_to_convert);

            assert_eq!(
                converted_value, expected_string,
                "Failed on input: '{string_to_convert}'",
            );
        }
    }
}
