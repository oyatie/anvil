use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSloSpec {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: OpenSloMetadata,
    pub spec: OpenSloBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSloMetadata {
    pub name: String,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSloBody {
    pub service: String,
    #[serde(rename = "indicator", default)]
    pub indicator: Option<serde_yaml::Value>,
    #[serde(rename = "objectives", default)]
    pub objectives: Vec<OpenSloObjective>,
    #[serde(rename = "timeWindow", default)]
    pub time_window: Vec<OpenSloTimeWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSloObjective {
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    pub target: f64,
    #[serde(rename = "timeSliceTarget", default)]
    pub time_slice_target: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSloTimeWindow {
    pub duration: String,
    #[serde(rename = "isRolling", default)]
    pub is_rolling: bool,
}

pub fn parse_openslo_yaml(content: &str) -> Result<OpenSloSpec> {
    serde_yaml::from_str::<OpenSloSpec>(content)
        .context("Failed to parse OpenSLO YAML specification")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_openslo() {
        let yaml = r#"
apiVersion: openslo/v1
kind: SLO
metadata:
  name: api-availability
  displayName: API Availability SLO
spec:
  service: console-app
  objectives:
    - displayName: 99.9% Availability
      target: 0.999
  timeWindow:
    - duration: 30d
      isRolling: true
"#;
        let spec = parse_openslo_yaml(yaml).expect("Parses valid OpenSLO");
        assert_eq!(spec.kind, "SLO");
        assert_eq!(spec.metadata.name, "api-availability");
        assert_eq!(spec.spec.service, "console-app");
        assert_eq!(spec.spec.objectives[0].target, 0.999);
    }
}
