use crate::{
    azure::app_configuration::dtos::{
        EnhancedFeatureFlag, EnhancedFeatureFlagAllocation,
        EnhancedFeatureFlagAllocationGroupAllocation,
        EnhancedFeatureFlagAllocationPercentileAllocation,
        EnhancedFeatureFlagAllocationUserAllocation, EnhancedFeatureFlagConditions,
        EnhancedFeatureFlagConditionsFeatureFilter, EnhancedFeatureFlagConditionsRequirementType,
        EnhancedFeatureFlagTelemetry, EnhancedFeatureFlagVariant,
        EnhancedFeatureFlagVariantStatusOverride,
    },
    core::content_type::ContentType,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Represents the input structure for the `featuremanagement` library, including feature flags.
///
/// Format reference:
///     - `https://github.com/microsoft/FeatureManagement/blob/main/Schema/FeatureManagement.v2.0.0.schema.json`.
///     - `https://github.com/microsoft/FeatureManagement/blob/main/Schema/FeatureFlag.v2.0.0.schema.json`.
#[derive(Debug, Serialize)]
pub(crate) struct FeatureManagementInput {
    feature_management: FeatureManagementInputFeatureManagement,
}

impl From<Vec<EnhancedFeatureFlag>> for FeatureManagementInput {
    fn from(feature_flags: Vec<EnhancedFeatureFlag>) -> Self {
        Self {
            feature_management: FeatureManagementInputFeatureManagement {
                feature_flags: feature_flags
                    .into_iter()
                    .map(FeatureManagementInputFeatureManagementFeatureFlag::from)
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagement {
    feature_flags: Vec<FeatureManagementInputFeatureManagementFeatureFlag>,
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlag {
    id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,

    enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    conditions: Option<FeatureManagementInputFeatureManagementFeatureFlagConditions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    variants: Option<Vec<FeatureManagementInputFeatureManagementFeatureFlagVariant>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    allocation: Option<FeatureManagementInputFeatureManagementFeatureFlagAllocation>,

    #[serde(skip_serializing_if = "Option::is_none")]
    telemetry: Option<FeatureManagementInputFeatureManagementFeatureFlagTelemetry>,
}

impl From<EnhancedFeatureFlag> for FeatureManagementInputFeatureManagementFeatureFlag {
    fn from(feature_flag: EnhancedFeatureFlag) -> Self {
        Self {
            id: feature_flag.name,
            description: feature_flag.description,
            display_name: None,
            enabled: feature_flag.enabled,
            conditions: feature_flag
                .conditions
                .map(FeatureManagementInputFeatureManagementFeatureFlagConditions::from),
            variants: feature_flag.variants.map(|variants| {
                variants
                    .into_iter()
                    .map(FeatureManagementInputFeatureManagementFeatureFlagVariant::from)
                    .collect()
            }),
            allocation: feature_flag
                .allocation
                .map(FeatureManagementInputFeatureManagementFeatureFlagAllocation::from),
            telemetry: feature_flag
                .telemetry
                .map(FeatureManagementInputFeatureManagementFeatureFlagTelemetry::from),
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagConditions {
    #[serde(skip_serializing_if = "Option::is_none")]
    requirement_type:
        Option<FeatureManagementInputFeatureManagementFeatureFlagConditionsRequirementType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    client_filters:
        Option<Vec<FeatureManagementInputFeatureManagementFeatureFlagConditionsClientFilter>>,
}

#[derive(Debug, Serialize)]
enum FeatureManagementInputFeatureManagementFeatureFlagConditionsRequirementType {
    Any,
    All,
}

impl From<EnhancedFeatureFlagConditions>
    for FeatureManagementInputFeatureManagementFeatureFlagConditions
{
    fn from(conditions: EnhancedFeatureFlagConditions) -> Self {
        Self {
            requirement_type: conditions.requirement_type.map(|requirement_type| {
                match requirement_type {
                    EnhancedFeatureFlagConditionsRequirementType::Any => {
                        FeatureManagementInputFeatureManagementFeatureFlagConditionsRequirementType::Any
                    }
                    EnhancedFeatureFlagConditionsRequirementType::All => {
                        FeatureManagementInputFeatureManagementFeatureFlagConditionsRequirementType::All
                    }
                }
            }),
            client_filters: conditions
                .filters
                .map(|filters| {
                    filters
                        .into_iter()
                        .map(FeatureManagementInputFeatureManagementFeatureFlagConditionsClientFilter::from)
                        .collect()
                }),
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagConditionsClientFilter {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<BTreeMap<String, Value>>,
}

impl From<EnhancedFeatureFlagConditionsFeatureFilter>
    for FeatureManagementInputFeatureManagementFeatureFlagConditionsClientFilter
{
    fn from(feature_filter: EnhancedFeatureFlagConditionsFeatureFilter) -> Self {
        Self {
            name: feature_filter.name,
            parameters: feature_filter.parameters,
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagVariant {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    configuration_value: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    status_override:
        Option<FeatureManagementInputFeatureManagementFeatureFlagVariantStatusOverride>,
}

#[derive(Debug, Serialize)]
enum FeatureManagementInputFeatureManagementFeatureFlagVariantStatusOverride {
    None,
    Enabled,
    Disabled,
}

impl From<EnhancedFeatureFlagVariant>
    for FeatureManagementInputFeatureManagementFeatureFlagVariant
{
    fn from(variant: EnhancedFeatureFlagVariant) -> Self {
        Self {
            name: variant.name,
            configuration_value: variant.value.map(|value| {
                if variant
                    .content_type
                    .is_some_and(|content_type| {
                        ContentType::new(&content_type).is_application_json()
                    })
                {
                    serde_json::from_str(&value).unwrap_or(Value::String(value))
                } else {
                    Value::String(value)
                }
            }),
            status_override: variant
                .status_override
                .map(|status_override| match status_override {
                    EnhancedFeatureFlagVariantStatusOverride::None => {
                        FeatureManagementInputFeatureManagementFeatureFlagVariantStatusOverride::None
                    }
                    EnhancedFeatureFlagVariantStatusOverride::Enabled => {
                        FeatureManagementInputFeatureManagementFeatureFlagVariantStatusOverride::Enabled
                    }
                    EnhancedFeatureFlagVariantStatusOverride::Disabled => {
                        FeatureManagementInputFeatureManagementFeatureFlagVariantStatusOverride::Disabled
                    }
                }),
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagAllocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    default_when_disabled: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    default_when_enabled: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<Vec<FeatureManagementInputFeatureManagementFeatureFlagAllocationUserAllocation>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<Vec<FeatureManagementInputFeatureManagementFeatureFlagAllocationGroupAllocation>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    percentile: Option<
        Vec<FeatureManagementInputFeatureManagementFeatureFlagAllocationPercentileAllocation>,
    >,

    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<String>,
}

impl From<EnhancedFeatureFlagAllocation>
    for FeatureManagementInputFeatureManagementFeatureFlagAllocation
{
    fn from(allocation: EnhancedFeatureFlagAllocation) -> Self {
        Self {
            default_when_disabled: allocation.default_when_disabled,
            default_when_enabled: allocation.default_when_enabled,
            user: allocation
                .user
                .map(|user| {
                    user.into_iter()
                        .map(FeatureManagementInputFeatureManagementFeatureFlagAllocationUserAllocation::from)
                        .collect()
                }),
            group: allocation
                .group
                .map(|group| {
                    group
                        .into_iter()
                        .map(FeatureManagementInputFeatureManagementFeatureFlagAllocationGroupAllocation::from)
                        .collect()
                }),
            percentile: allocation
                .percentile
                .map(|percentile| {
                    percentile
                        .into_iter()
                        .map(FeatureManagementInputFeatureManagementFeatureFlagAllocationPercentileAllocation::from)
                        .collect()
                }),
            seed: allocation.seed,
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagAllocationPercentileAllocation {
    variant: String,
    from: u8,
    to: u8,
}

impl From<EnhancedFeatureFlagAllocationPercentileAllocation>
    for FeatureManagementInputFeatureManagementFeatureFlagAllocationPercentileAllocation
{
    fn from(allocation: EnhancedFeatureFlagAllocationPercentileAllocation) -> Self {
        Self {
            variant: allocation.variant,
            from: allocation.from,
            to: allocation.to,
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagAllocationUserAllocation {
    variant: String,
    users: Vec<String>,
}

impl From<EnhancedFeatureFlagAllocationUserAllocation>
    for FeatureManagementInputFeatureManagementFeatureFlagAllocationUserAllocation
{
    fn from(allocation: EnhancedFeatureFlagAllocationUserAllocation) -> Self {
        Self {
            variant: allocation.variant,
            users: allocation.users,
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagAllocationGroupAllocation {
    variant: String,
    groups: Vec<String>,
}

impl From<EnhancedFeatureFlagAllocationGroupAllocation>
    for FeatureManagementInputFeatureManagementFeatureFlagAllocationGroupAllocation
{
    fn from(allocation: EnhancedFeatureFlagAllocationGroupAllocation) -> Self {
        Self {
            variant: allocation.variant,
            groups: allocation.groups,
        }
    }
}

#[derive(Debug, Serialize)]
struct FeatureManagementInputFeatureManagementFeatureFlagTelemetry {
    enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<BTreeMap<String, String>>,
}

impl From<EnhancedFeatureFlagTelemetry>
    for FeatureManagementInputFeatureManagementFeatureFlagTelemetry
{
    fn from(telemetry: EnhancedFeatureFlagTelemetry) -> Self {
        Self {
            enabled: telemetry.enabled,
            metadata: telemetry.metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::azure::app_configuration::{
        dtos::{
            EnhancedFeatureFlag, EnhancedFeatureFlagAllocation,
            EnhancedFeatureFlagAllocationGroupAllocation,
            EnhancedFeatureFlagAllocationPercentileAllocation,
            EnhancedFeatureFlagAllocationUserAllocation, EnhancedFeatureFlagConditions,
            EnhancedFeatureFlagConditionsFeatureFilter,
            EnhancedFeatureFlagConditionsRequirementType, EnhancedFeatureFlagTelemetry,
            EnhancedFeatureFlagVariant, EnhancedFeatureFlagVariantStatusOverride,
        },
        feature_management_input::{
            FeatureManagementInput, FeatureManagementInputFeatureManagementFeatureFlag,
        },
    };

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_serializes_complete_feature_management_schema() {
        let input = FeatureManagementInput::from(vec![EnhancedFeatureFlag {
            name: String::from("feature"),
            description: Some(String::from("Feature description")),
            enabled: true,
            conditions: Some(EnhancedFeatureFlagConditions {
                requirement_type: Some(EnhancedFeatureFlagConditionsRequirementType::All),
                filters: Some(vec![EnhancedFeatureFlagConditionsFeatureFilter {
                    name: String::from("Filter"),
                    parameters: Some(BTreeMap::from([
                        (String::from("string"), Value::String(String::from("value"))),
                        (String::from("null"), Value::Null),
                        (
                            String::from("object"),
                            serde_json::json!({ "nested": "value" }),
                        ),
                        (String::from("number"), serde_json::json!(1)),
                        (String::from("array"), serde_json::json!(["value"])),
                        (String::from("boolean"), Value::Bool(true)),
                    ])),
                }]),
            }),
            variants: Some(vec![
                EnhancedFeatureFlagVariant {
                    name: String::from("variant"),
                    value: Some(String::from("{\"key\":\"value\"}")),
                    content_type: Some(String::from("application/json")),
                    status_override: Some(EnhancedFeatureFlagVariantStatusOverride::Enabled),
                },
                EnhancedFeatureFlagVariant {
                    name: String::from("numeric_variant"),
                    value: Some(String::from("1")),
                    content_type: Some(String::from("application/json")),
                    status_override: None,
                },
            ]),
            allocation: Some(EnhancedFeatureFlagAllocation {
                default_when_disabled: Some(String::from("disabled")),
                default_when_enabled: Some(String::from("enabled")),
                percentile: Some(vec![EnhancedFeatureFlagAllocationPercentileAllocation {
                    variant: String::from("variant"),
                    from: 0,
                    to: 100,
                }]),
                user: Some(vec![EnhancedFeatureFlagAllocationUserAllocation {
                    variant: String::from("variant"),
                    users: vec![String::from("user")],
                }]),
                group: Some(vec![EnhancedFeatureFlagAllocationGroupAllocation {
                    variant: String::from("variant"),
                    groups: vec![String::from("group")],
                }]),
                seed: Some(String::from("seed")),
            }),
            telemetry: Some(EnhancedFeatureFlagTelemetry {
                enabled: true,
                metadata: Some(BTreeMap::from([(
                    String::from("key"),
                    String::from("value"),
                )])),
            }),
        }]);

        let serialized = serde_json::to_string(&input).unwrap();
        let document: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            document,
            serde_json::json!({
                "feature_management": {
                    "feature_flags": [{
                        "id": "feature",
                        "description": "Feature description",
                        "enabled": true,
                        "conditions": {
                            "requirement_type": "All",
                            "client_filters": [{
                                "name": "Filter",
                                "parameters": {
                                    "string": "value",
                                    "null": null,
                                    "object": { "nested": "value" },
                                    "number": 1,
                                    "array": ["value"],
                                    "boolean": true
                                }
                            }]
                        },
                        "variants": [{
                            "name": "variant",
                            "configuration_value": { "key": "value" },
                            "status_override": "Enabled"
                        }, {
                            "name": "numeric_variant",
                            "configuration_value": 1
                        }],
                        "allocation": {
                            "default_when_disabled": "disabled",
                            "default_when_enabled": "enabled",
                            "percentile": [{ "variant": "variant", "from": 0, "to": 100 }],
                            "user": [{ "variant": "variant", "users": ["user"] }],
                            "group": [{ "variant": "variant", "groups": ["group"] }],
                            "seed": "seed"
                        },
                        "telemetry": { "enabled": true, "metadata": { "key": "value" } }
                    }]
                }
            })
        );
    }

    #[test]
    fn test_preserves_none_status_override_and_string_variant_value() {
        let input = FeatureManagementInput::from(vec![EnhancedFeatureFlag {
            name: String::from("feature"),
            description: None,
            enabled: false,
            conditions: None,
            variants: Some(vec![EnhancedFeatureFlagVariant {
                name: String::from("variant"),
                value: Some(String::from("value")),
                content_type: Some(String::from("text/plain")),
                status_override: Some(EnhancedFeatureFlagVariantStatusOverride::None),
            }]),
            allocation: None,
            telemetry: None,
        }]);

        let serialized = serde_json::to_string(&input).unwrap();
        let feature_flags: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            feature_flags,
            serde_json::json!({
                "feature_management": {
                    "feature_flags": [{
                        "id": "feature",
                        "enabled": false,
                        "variants": [{
                            "name": "variant",
                            "configuration_value": "value",
                            "status_override": "None"
                        }]
                    }]
                }
            })
        );
    }

    #[test]
    fn test_serializes_display_name_when_provided() {
        let feature_flag = FeatureManagementInputFeatureManagementFeatureFlag {
            id: String::from("feature"),
            description: None,
            display_name: Some(String::from("Feature display name")),
            enabled: false,
            conditions: None,
            variants: None,
            allocation: None,
            telemetry: None,
        };

        let serialized = serde_json::to_value(&feature_flag).unwrap();

        assert_eq!(
            serialized,
            serde_json::json!({
                "id": "feature",
                "display_name": "Feature display name",
                "enabled": false,
            })
        );
    }

    #[test]
    fn test_serializes_any_requirement_type() {
        let input = FeatureManagementInput::from(vec![EnhancedFeatureFlag {
            name: String::from("feature"),
            description: None,
            enabled: true,
            conditions: Some(EnhancedFeatureFlagConditions {
                requirement_type: Some(EnhancedFeatureFlagConditionsRequirementType::Any),
                filters: None,
            }),
            variants: None,
            allocation: None,
            telemetry: None,
        }]);

        let serialized = serde_json::to_value(&input).unwrap();

        assert_eq!(
            serialized,
            serde_json::json!({
                "feature_management": {
                    "feature_flags": [{
                        "id": "feature",
                        "enabled": true,
                        "conditions": { "requirement_type": "Any" }
                    }]
                }
            })
        );
    }
}
