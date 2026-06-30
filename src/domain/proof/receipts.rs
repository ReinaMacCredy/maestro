use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::card::live_db;
use crate::domain::card::locator::ArtifactLocator;
use crate::foundation::core::paths::MaestroPaths;

pub const RECONCILE_RECEIPT_TYPE: &str = "reconcile_receipt";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReceiptExtension {
    pub artifact: ArtifactLocator,
    pub card_id: Option<String>,
    pub created_at: String,
    pub payload_json: String,
}

pub fn store_receipt_extension(
    paths: &MaestroPaths,
    artifact_type: &str,
    artifact_id: &str,
    card_id: Option<&str>,
    payload_json: &str,
) -> Result<ReceiptExtension> {
    let stored =
        live_db::record_receipt_artifact(paths, artifact_type, artifact_id, card_id, payload_json)?;
    Ok(from_stored(paths, stored))
}

pub fn load_receipt_extension(
    paths: &MaestroPaths,
    artifact_type: &str,
    artifact_id: &str,
) -> Result<Option<ReceiptExtension>> {
    Ok(
        live_db::load_receipt_artifact(paths, artifact_type, artifact_id)?
            .map(|stored| from_stored(paths, stored)),
    )
}

pub fn store_reconcile_receipt_extension(
    paths: &MaestroPaths,
    artifact_id: &str,
    card_id: Option<&str>,
    payload_json: &str,
) -> Result<ReceiptExtension> {
    store_receipt_extension(
        paths,
        RECONCILE_RECEIPT_TYPE,
        artifact_id,
        card_id,
        payload_json,
    )
}

fn from_stored(paths: &MaestroPaths, stored: live_db::StoredReceiptArtifact) -> ReceiptExtension {
    let artifact = ArtifactLocator::new(&stored.artifact_type, &stored.id)
        .with_locator("store_id", paths.store_db_file().display().to_string())
        .with_locator("table", "receipt_artifacts")
        .with_locator("key", format!("{}:{}", stored.artifact_type, stored.id));
    ReceiptExtension {
        artifact,
        card_id: stored.card_id,
        created_at: stored.created_at,
        payload_json: stored.payload_json,
    }
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_paths(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-proof-receipts-{label}-{}-{nanos}",
            process::id()
        ));
        MaestroPaths::new(root)
    }

    #[test]
    fn receipt_extension_round_trips_through_store_db() {
        let paths = temp_paths("round-trip");
        let payload = r#"{"state":"current","stale":[]}"#;

        let stored =
            store_reconcile_receipt_extension(&paths, "receipt-1", Some("feature-a"), payload)
                .expect("store receipt extension");
        assert_eq!(stored.artifact.artifact_type, RECONCILE_RECEIPT_TYPE);
        assert_eq!(stored.artifact.id, "receipt-1");
        assert_eq!(stored.card_id.as_deref(), Some("feature-a"));

        let loaded = load_receipt_extension(&paths, RECONCILE_RECEIPT_TYPE, "receipt-1")
            .expect("load receipt")
            .expect("receipt exists");
        assert_eq!(loaded.artifact, stored.artifact);
        assert_eq!(loaded.payload_json, payload);

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }
}
