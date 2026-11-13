//! Local model metadata catalog used by `--list_models`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub name: String,
    pub filename: String,
    pub architecture: String,
    pub stems: Vec<String>,
    #[serde(rename = "SDR", default, skip_serializing_if = "Option::is_none")]
    pub sdr: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// Instructions for obtaining and preparing the weights; no weights are bundled.
    #[serde(default)]
    pub weights: String,
}

pub fn builtin_models() -> Result<Vec<ModelInfo>, String> {
    serde_json::from_str(include_str!("../data/models.json"))
        .map_err(|error| format!("cannot parse built-in model catalog: {error}"))
}

/// Load a user manifest containing either a JSON/YAML array or `{ "models": [...] }`.
pub fn load_models(path: &Path) -> Result<Vec<ModelInfo>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read model manifest {}: {error}", path.display()))?;
    let value: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|error| format!("cannot parse model manifest {}: {error}", path.display()))?;
    let rows = match &value {
        serde_yaml::Value::Sequence(_) => value,
        serde_yaml::Value::Mapping(map) => map
            .get(serde_yaml::Value::String("models".into()))
            .cloned()
            .ok_or_else(|| "model manifest object must contain a `models` array".to_string())?,
        _ => return Err("model manifest must be an array or an object with `models`".into()),
    };
    let models: Vec<ModelInfo> = serde_yaml::from_value(rows)
        .map_err(|error| format!("invalid model manifest {}: {error}", path.display()))?;
    validate_models(&models)?;
    Ok(models)
}

pub fn validate_models(models: &[ModelInfo]) -> Result<(), String> {
    for (index, model) in models.iter().enumerate() {
        if model.name.trim().is_empty()
            || model.filename.trim().is_empty()
            || model.architecture.trim().is_empty()
            || model.stems.is_empty()
            || model.stems.iter().any(|stem| stem.trim().is_empty())
        {
            return Err(format!(
                "model entry {} is missing required metadata",
                index + 1
            ));
        }
    }
    Ok(())
}

pub fn list_models(
    models: &[ModelInfo],
    filter: Option<&str>,
    limit: Option<usize>,
) -> Vec<ModelInfo> {
    let filter = filter.map(str::to_ascii_lowercase);
    models
        .iter()
        .filter(|model| {
            filter.as_ref().is_none_or(|filter| {
                format!(
                    "{} {} {} {}",
                    model.name,
                    model.filename,
                    model.architecture,
                    model.stems.join(" ")
                )
                .to_ascii_lowercase()
                .contains(filter)
            })
        })
        .take(limit.unwrap_or(usize::MAX))
        .cloned()
        .collect()
}

pub fn render_pretty(models: &[ModelInfo]) -> String {
    let mut out =
        String::from("NAME | FILENAME | ARCHITECTURE | STEMS | SDR | CHECKSUM | WEIGHTS\n");
    for model in models {
        out.push_str(&format!(
            "{} | {} | {} | {} | {} | {} | {}\n",
            model.name,
            model.filename,
            model.architecture,
            model.stems.join(","),
            model
                .sdr
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into()),
            model.checksum.as_deref().unwrap_or("-"),
            model.weights
        ));
    }
    out
}

pub fn render_json(models: &[ModelInfo]) -> Result<String, String> {
    serde_json::to_string_pretty(models)
        .map_err(|error| format!("cannot serialize model catalog: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_local_filterable_and_stable_json() {
        let models = builtin_models().unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(list_models(&models, Some("viperx"), None).len(), 1);
        let json = render_json(&models).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value[0]["name"], models[0].name);
        assert!(render_pretty(&models).starts_with("NAME | FILENAME | ARCHITECTURE"));
    }
}
