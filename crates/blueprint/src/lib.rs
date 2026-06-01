//! Plutus blueprint (CIP-57) reader.
//!
//! Reads `<project_root>/plutus.json` produced by `aiken build`. Parses
//! validators, parameter schemas, and (when present) compiled-script size.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;

use aiken_mcp_core::{
    Blueprint, BlueprintParam, BlueprintReader, BlueprintValidator, CoreError, CoreResult,
};

#[derive(Debug, Clone, Default)]
pub struct JsonBlueprintReader;

impl JsonBlueprintReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BlueprintReader for JsonBlueprintReader {
    async fn read(&self, project_root: &Path) -> CoreResult<Blueprint> {
        let path: PathBuf = project_root.join("plutus.json");
        let text = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CoreError::other(format!(
                    "plutus.json not found at {} (run `aiken build` first)",
                    path.display()
                ))
            } else {
                CoreError::Io(e)
            }
        })?;

        let raw: serde_json::Value = serde_json::from_str(&text).map_err(CoreError::Serde)?;
        let parsed: BlueprintFile =
            serde_json::from_value(raw.clone()).map_err(CoreError::Serde)?;

        let validators = parsed
            .validators
            .into_iter()
            .map(|v| BlueprintValidator {
                title: v.title,
                hash: v.hash,
                compiled_size_bytes: v.compiled_code.as_ref().map(|hex| hex.len() / 2),
                parameters: v
                    .parameters
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| BlueprintParam {
                        title: p.title,
                        schema: p.schema.unwrap_or(serde_json::Value::Null),
                    })
                    .collect(),
                datum: v.datum.map(|d| d.schema.unwrap_or(serde_json::Value::Null)),
                redeemer: v
                    .redeemer
                    .map(|d| d.schema.unwrap_or(serde_json::Value::Null)),
            })
            .collect();

        Ok(Blueprint { validators, raw })
    }
}

#[derive(Debug, Deserialize)]
struct BlueprintFile {
    #[serde(default)]
    validators: Vec<RawValidator>,
}

#[derive(Debug, Deserialize)]
struct RawValidator {
    title: String,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default, rename = "compiledCode")]
    compiled_code: Option<String>,
    #[serde(default)]
    parameters: Option<Vec<RawParam>>,
    #[serde(default)]
    datum: Option<RawSchemaWrapper>,
    #[serde(default)]
    redeemer: Option<RawSchemaWrapper>,
}

#[derive(Debug, Deserialize)]
struct RawParam {
    title: String,
    #[serde(default)]
    schema: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawSchemaWrapper {
    #[serde(default)]
    schema: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn parses_minimal_blueprint() {
        let tmp = tempdir().unwrap();
        let blueprint = serde_json::json!({
            "validators": [
                {
                    "title": "foo.spend",
                    "hash": "deadbeef",
                    "compiledCode": "abcd1234"
                }
            ]
        });
        write(
            tmp.path().join("plutus.json"),
            serde_json::to_string(&blueprint).unwrap(),
        )
        .unwrap();
        let reader = JsonBlueprintReader::new();
        let bp = reader.read(tmp.path()).await.unwrap();
        assert_eq!(bp.validators.len(), 1);
        assert_eq!(bp.validators[0].title, "foo.spend");
        assert_eq!(bp.validators[0].compiled_size_bytes, Some(4));
    }

    #[tokio::test]
    async fn errors_when_missing() {
        let tmp = tempdir().unwrap();
        let reader = JsonBlueprintReader::new();
        let err = reader.read(tmp.path()).await.unwrap_err();
        assert!(err.to_string().contains("plutus.json not found"));
    }
}
