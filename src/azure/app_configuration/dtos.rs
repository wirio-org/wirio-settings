use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub(crate) struct GetConfigurationsResponse {
    pub(crate) items: Vec<Configuration>,

    #[serde(rename = "@nextLink")]
    pub(crate) next_link: Option<String>,
}

/// Key-value pair representing a configuration setting.
///
/// Format reference: `https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-key-value?pivots=v26-05-preview#syntax`.
#[derive(Debug, Deserialize)]
pub(crate) struct Configuration {
    pub(crate) key: String,
    pub(crate) content_type: Option<String>,
    pub(crate) value: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetEnhancedFeatureFlagsResponse {
    pub(crate) items: Vec<EnhancedFeatureFlag>,

    #[serde(rename = "@nextLink")]
    pub(crate) next_link: Option<String>,
}

/// Format reference: `https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-enhanced-feature-flag?pivots=v26-05-preview#feature-flag`.
#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlag {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) description: Option<String>,
    pub(crate) conditions: Option<EnhancedFeatureFlagConditions>,
    pub(crate) variants: Option<Vec<EnhancedFeatureFlagVariant>>,
    pub(crate) allocation: Option<EnhancedFeatureFlagAllocation>,
    pub(crate) telemetry: Option<EnhancedFeatureFlagTelemetry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagConditions {
    pub(crate) requirement_type: Option<EnhancedFeatureFlagConditionsRequirementType>,
    pub(crate) filters: Option<Vec<EnhancedFeatureFlagConditionsFeatureFilter>>,
}

#[derive(Debug, Deserialize)]
pub(crate) enum EnhancedFeatureFlagConditionsRequirementType {
    Any,
    All,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagConditionsFeatureFilter {
    pub(crate) name: String,
    pub(crate) parameters: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagVariant {
    pub(crate) name: String,
    pub(crate) value: Option<String>,
    pub(crate) content_type: Option<String>,
    pub(crate) status_override: Option<EnhancedFeatureFlagVariantStatusOverride>,
}

#[derive(Debug, Deserialize)]
pub(crate) enum EnhancedFeatureFlagVariantStatusOverride {
    None,
    Enabled,
    Disabled,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagAllocation {
    pub(crate) default_when_disabled: Option<String>,
    pub(crate) default_when_enabled: Option<String>,
    pub(crate) percentile: Option<Vec<EnhancedFeatureFlagAllocationPercentileAllocation>>,
    pub(crate) user: Option<Vec<EnhancedFeatureFlagAllocationUserAllocation>>,
    pub(crate) group: Option<Vec<EnhancedFeatureFlagAllocationGroupAllocation>>,
    pub(crate) seed: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagAllocationPercentileAllocation {
    pub(crate) variant: String,
    pub(crate) from: u8,
    pub(crate) to: u8,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagAllocationUserAllocation {
    pub(crate) variant: String,
    pub(crate) users: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagAllocationGroupAllocation {
    pub(crate) variant: String,
    pub(crate) groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnhancedFeatureFlagTelemetry {
    pub(crate) enabled: bool,
    pub(crate) metadata: Option<BTreeMap<String, String>>,
}
