// Represents an HTTP Content-Type header.
pub(crate) struct ContentType<'a> {
    value: &'a str,
}

impl<'a> ContentType<'a> {
    const APPLICATION_JSON_MEDIA_TYPE: &'static str = "application/json";
    const KEY_VAULT_REFERENCE_MEDIA_TYPE: &'static str =
        "application/vnd.microsoft.appconfig.keyvaultref+json";
    const SNAPSHOT_REFERENCE_PROFILE: &'static str =
        "https://azconfig.io/mime-profiles/snapshot-ref";

    pub(crate) fn new(value: &'a str) -> Self {
        Self { value }
    }

    pub(crate) fn is_key_vault_reference(&self) -> bool {
        self.media_type()
            .eq_ignore_ascii_case(Self::KEY_VAULT_REFERENCE_MEDIA_TYPE)
    }

    pub(crate) fn is_application_json(&self) -> bool {
        self.media_type()
            .eq_ignore_ascii_case(Self::APPLICATION_JSON_MEDIA_TYPE)
    }

    pub(crate) fn is_snapshot_reference(&self) -> bool {
        self.is_application_json()
            && self.has_parameter("profile", Self::SNAPSHOT_REFERENCE_PROFILE)
    }

    fn media_type(&self) -> &str {
        self.value
            .split_once(';')
            .map_or(self.value, |(value, _)| value)
    }

    fn has_parameter(&self, name: &str, value: &str) -> bool {
        self.value
            .split(';')
            .map(str::trim)
            .filter_map(|parameter| parameter.split_once('='))
            .any(|(parameter_name, parameter_value)| {
                parameter_name.eq_ignore_ascii_case(name)
                    && parameter_value.trim_matches('"') == value
            })
    }
}

#[cfg(test)]
mod tests {
    use super::ContentType;

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

    #[test]
    fn test_identify_snapshot_reference_with_profile() {
        assert!(ContentType::new(
            "application/json; profile=\"https://azconfig.io/mime-profiles/snapshot-ref\"; charset=utf-8"
        )
        .is_snapshot_reference());
    }

    #[test]
    fn test_identify_media_types_ignoring_case() {
        assert!(ContentType::new("Application/Json").is_application_json());
        assert!(
            ContentType::new("Application/Vnd.Microsoft.AppConfig.KeyVaultRef+Json")
                .is_key_vault_reference()
        );
        assert!(
            ContentType::new(
                "Application/Json; profile=\"https://azconfig.io/mime-profiles/snapshot-ref\""
            )
            .is_snapshot_reference()
        );
    }

    #[test]
    fn test_get_media_type_for_ai_setting() {
        assert_eq!(
            ContentType::new(
                "application/json; profile=\"https://azconfig.io/mime-profiles/ai/chat-completion\""
            )
            .media_type(),
            "application/json"
        );
    }
}
