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
    use std::collections::HashSet;

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

    #[test]
    fn fixture_catalog_filter_limit_pretty_columns_and_json_schema_are_stable() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("model-catalog.json");
        let models = load_models(&fixture).unwrap();
        assert_eq!(models.len(), 3);

        let filtered = list_models(&models, Some("MELBANDROFORMER"), Some(1));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Fixture RoseRoformer vocals");
        let bass = list_models(&models, Some("bass"), None);
        assert_eq!(bass.len(), 1);
        assert_eq!(bass[0].filename, "fixture-bass.safetensors");

        let pretty = render_pretty(&models);
        assert_eq!(
            pretty.lines().next(),
            Some("NAME | FILENAME | ARCHITECTURE | STEMS | SDR | CHECKSUM | WEIGHTS")
        );
        assert_eq!(pretty.lines().count(), 4);
        assert!(pretty.contains("Fixture RoseRoformer vocals | fixture-rose.safetensors"));

        let json = render_json(&filtered).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let row = value[0].as_object().unwrap();
        let actual_keys: HashSet<&str> = row.keys().map(String::as_str).collect();
        let expected_keys: HashSet<&str> = [
            "name",
            "filename",
            "architecture",
            "stems",
            "SDR",
            "checksum",
            "weights",
        ]
        .into_iter()
        .collect();
        assert_eq!(actual_keys, expected_keys);
        assert_eq!(row["name"], "Fixture RoseRoformer vocals");
        assert_eq!(row["SDR"], 7.25);
        assert!(
            row.get("sdr").is_none(),
            "JSON schema keeps the published SDR key"
        );
    }
}
