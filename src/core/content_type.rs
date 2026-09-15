pub(crate) const APPLICATION_JSON_MEDIA_TYPE: &str = "application/json";

pub(crate) fn get_media_type(content_type: &str) -> &str {
    content_type
        .split_once(';')
        .map_or(content_type, |(value, _)| value)
}

#[cfg(test)]
mod tests {
    use crate::core::content_type::get_media_type;

    #[test]
    fn test_get_media_type_without_parameters() {
        assert_eq!(get_media_type("application/json"), "application/json");
    }

    #[test]
    fn test_get_media_type_with_parameters_and_spaces() {
        assert_eq!(
            get_media_type("application/json; charset=utf-8"),
            "application/json"
        );
    }

    #[test]
    fn test_get_media_type_for_key_vault_reference() {
        assert_eq!(
            get_media_type("application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8"),
            "application/vnd.microsoft.appconfig.keyvaultref+json"
        );
    }

    #[test]
    fn test_get_media_type_for_feature_flags() {
        assert_eq!(
            get_media_type(
                r#"application/json; profile="https://azconfig.io/mime-profiles/ffset"; charset=utf-8"#
            ),
            "application/json"
        );
    }
}
