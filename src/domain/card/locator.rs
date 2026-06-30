use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::foundation::core::paths::MaestroPaths;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    CardFolder,
    Workbench,
    Db,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceBackend {
    Filesystem,
    Sqlite,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceLocator {
    pub kind: SurfaceKind,
    pub feature_id: String,
    pub backend: SurfaceBackend,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locator: BTreeMap<String, String>,
}

impl SurfaceLocator {
    pub fn card_folder(paths: &MaestroPaths, feature_id: &str) -> Self {
        Self::filesystem(
            SurfaceKind::CardFolder,
            feature_id,
            paths.cards_dir().join(feature_id).display().to_string(),
        )
    }

    pub fn workbench(paths: &MaestroPaths, feature_id: &str) -> Self {
        Self::filesystem(
            SurfaceKind::Workbench,
            feature_id,
            paths.workbench_dir().join(feature_id).display().to_string(),
        )
    }

    pub fn db(paths: &MaestroPaths, feature_id: &str) -> Self {
        let mut locator = BTreeMap::new();
        locator.insert(
            "store_id".to_string(),
            paths.store_db_file().display().to_string(),
        );
        locator.insert("table".to_string(), "cards".to_string());
        locator.insert("key".to_string(), feature_id.to_string());
        Self {
            kind: SurfaceKind::Db,
            feature_id: feature_id.to_string(),
            backend: SurfaceBackend::Sqlite,
            locator,
        }
    }

    fn filesystem(kind: SurfaceKind, feature_id: &str, path: String) -> Self {
        let mut locator = BTreeMap::new();
        locator.insert("path".to_string(), path);
        Self {
            kind,
            feature_id: feature_id.to_string(),
            backend: SurfaceBackend::Filesystem,
            locator,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactLocator {
    #[serde(rename = "type")]
    pub artifact_type: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locator: BTreeMap<String, String>,
}

impl ArtifactLocator {
    pub fn new(artifact_type: &str, id: &str) -> Self {
        Self {
            artifact_type: artifact_type.to_string(),
            id: id.to_string(),
            locator: BTreeMap::new(),
        }
    }

    pub fn with_locator(mut self, key: &str, value: impl Into<String>) -> Self {
        self.locator.insert(key.to_string(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_locator_json_is_backend_neutral() {
        let paths = MaestroPaths::new("/repo");
        let db = SurfaceLocator::db(&paths, "feat-a");
        let json = serde_json::to_value(&db).expect("serialize locator");

        assert_eq!(json["kind"], "db");
        assert_eq!(json["backend"], "sqlite");
        assert_eq!(json["feature_id"], "feat-a");
        assert_eq!(json["locator"]["table"], "cards");
        assert_eq!(json["locator"]["key"], "feat-a");
    }

    #[test]
    fn artifact_locator_uses_type_id_identity() {
        let artifact = ArtifactLocator::new("reconcile_receipt", "rec-1")
            .with_locator("table", "receipt_artifacts");
        let json = serde_json::to_value(&artifact).expect("serialize artifact");

        assert_eq!(json["type"], "reconcile_receipt");
        assert_eq!(json["id"], "rec-1");
        assert_eq!(json["locator"]["table"], "receipt_artifacts");
    }
}
