// Represents an HTTP Content-Type header.
pub(crate) struct ContentType<'a> {
    value: &'a str,
}

impl<'a> ContentType<'a> {
    const APPLICATION_JSON_MEDIA_TYPE: &'static str = "application/json";
    const KEY_VAULT_REFERENCE_MEDIA_TYPE: &'static str =
        "application/vnd.microsoft.appconfig.keyvaultref+json";

    pub(crate) fn new(value: &'a str) -> Self {
        Self { value }
    }

    pub(crate) fn media_type(&self) -> &str {
        self.value
            .split_once(';')
            .map_or(self.value, |(value, _)| value)
    }

    pub(crate) fn is_key_vault_reference(&self) -> bool {
        self.media_type() == Self::KEY_VAULT_REFERENCE_MEDIA_TYPE
    }

    pub(crate) fn is_application_json(&self) -> bool {
        self.media_type() == Self::APPLICATION_JSON_MEDIA_TYPE
    }
}

#[cfg(test)]
mod tests {
    use crate::core::content_type::ContentType;

    #[test]
    fn test_get_media_type_without_parameters() {
        assert_eq!(
            ContentType::new("application/json").media_type(),
            "application/json"
        );
    }

    #[test]
    fn test_get_media_type_without_parameters_and_spaces() {
        assert_eq!(
            ContentType::new("application/json; charset=utf-8").media_type(),
            "application/json"
        );
    }

    #[test]
    fn test_get_media_type_for_feature_flags() {
        assert_eq!(
            ContentType::new(
                r#"application/json; profile="https://azconfig.io/mime-profiles/ffset"; charset=utf-8"#
            )
            .media_type(),
            "application/json"
        );
    }

    #[test]
    fn test_identify_key_vault_reference() {
        assert!(
            ContentType::new("application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8")
                .is_key_vault_reference()
        );
    }

    #[test]
    fn test_identify_json_media_type() {
        assert!(ContentType::new("application/json; charset=utf-8").is_application_json());
    }

    #[test]
    fn test_get_media_type_for_empty_value() {
        assert_eq!(ContentType::new("").media_type(), "");
    }
}
