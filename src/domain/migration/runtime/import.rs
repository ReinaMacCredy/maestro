use thiserror::Error;

use crate::domain::identity::StoreObjectIdV1;
use crate::domain::persistence::{ExportError, RestoreCandidateV1, SealedBackupV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    ByteTotalInventoryV1, ClassificationSetV1, ConsumerClosureV1, ConsumerGateStageV1,
    DeterministicIdentityMapV1, MigrationDigestV1, MigrationIdentityErrorV1,
    SealedQuarantineManifestV1,
};

const IMPORT_REQUEST_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.inactive-import-request.v1\0";
const IMPORT_RECEIPT_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.inactive-import-receipt.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InactiveStoreImportRequestV1 {
    inventory_id: MigrationDigestV1,
    classification_set_id: MigrationDigestV1,
    target_set_id: MigrationDigestV1,
    quarantine_set_id: MigrationDigestV1,
    consumer_set_id: MigrationDigestV1,
    consumer_census_id: MigrationDigestV1,
    protocol_closure_id: MigrationDigestV1,
    sealed_backup_byte_length: u64,
    sealed_backup_sha256: MigrationDigestV1,
    expected_candidate_root_id: StoreObjectIdV1,
    id: MigrationDigestV1,
}

impl InactiveStoreImportRequestV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the inactive import binds every completed migration proof set"
    )]
    pub fn new(
        inventory: &ByteTotalInventoryV1,
        classifications: &ClassificationSetV1,
        target_map: &DeterministicIdentityMapV1,
        quarantine: &SealedQuarantineManifestV1,
        consumers: &ConsumerClosureV1,
        sealed_backup: &[u8],
        expected_candidate_root_id: StoreObjectIdV1,
    ) -> Result<Self, InactiveImportErrorV1> {
        if classifications.inventory_id() != inventory.id()
            || target_map.classification_set_id() != classifications.id()
            || quarantine.inventory_id() != inventory.id()
            || quarantine.classification_set_id() != classifications.id()
        {
            return Err(InactiveImportErrorV1::ProofSetMismatch);
        }
        if consumers.stage() != ConsumerGateStageV1::BeforeSemanticCurrentness
            || !consumers.gate_passed()
            || consumers.census().entries().is_empty()
        {
            return Err(InactiveImportErrorV1::ConsumerGateNotClosed);
        }
        if sealed_backup.is_empty() {
            return Err(InactiveImportErrorV1::EmptySealedBackup);
        }
        let sealed_backup_byte_length = u64::try_from(sealed_backup.len())
            .map_err(|_| InactiveImportErrorV1::SealedBackupLengthOverflow)?;
        let sealed_backup_sha256 = MigrationDigestV1::digest_bytes(sealed_backup)?;
        let value = CborValue::Array(vec![
            inventory.id().canonical_value(),
            classifications.id().canonical_value(),
            target_map.id().canonical_value(),
            quarantine.id().canonical_value(),
            consumers.id().canonical_value(),
            consumers.census().id().canonical_value(),
            consumers.protocol().id().canonical_value(),
            CborValue::Unsigned(sealed_backup_byte_length),
            sealed_backup_sha256.canonical_value(),
            CborValue::Bytes(expected_candidate_root_id.as_bytes().to_vec()),
        ]);
        let id = MigrationDigestV1::identify(IMPORT_REQUEST_DOMAIN_V1, &value)?;
        Ok(Self {
            inventory_id: inventory.id(),
            classification_set_id: classifications.id(),
            target_set_id: target_map.id(),
            quarantine_set_id: quarantine.id(),
            consumer_set_id: consumers.id(),
            consumer_census_id: consumers.census().id(),
            protocol_closure_id: consumers.protocol().id(),
            sealed_backup_byte_length,
            sealed_backup_sha256,
            expected_candidate_root_id,
            id,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn inventory_id(&self) -> MigrationDigestV1 {
        self.inventory_id
    }

    pub const fn classification_set_id(&self) -> MigrationDigestV1 {
        self.classification_set_id
    }

    pub const fn target_set_id(&self) -> MigrationDigestV1 {
        self.target_set_id
    }

    pub const fn quarantine_set_id(&self) -> MigrationDigestV1 {
        self.quarantine_set_id
    }

    pub const fn consumer_set_id(&self) -> MigrationDigestV1 {
        self.consumer_set_id
    }

    pub const fn consumer_census_id(&self) -> MigrationDigestV1 {
        self.consumer_census_id
    }

    pub const fn protocol_closure_id(&self) -> MigrationDigestV1 {
        self.protocol_closure_id
    }

    pub const fn expected_candidate_root_id(&self) -> StoreObjectIdV1 {
        self.expected_candidate_root_id
    }

    pub const fn sealed_backup_sha256(&self) -> MigrationDigestV1 {
        self.sealed_backup_sha256
    }

    pub fn verify_sealed_backup(&self, sealed_backup: &[u8]) -> Result<(), InactiveImportErrorV1> {
        let observed_length = u64::try_from(sealed_backup.len())
            .map_err(|_| InactiveImportErrorV1::SealedBackupLengthOverflow)?;
        if observed_length != self.sealed_backup_byte_length
            || MigrationDigestV1::digest_bytes(sealed_backup)? != self.sealed_backup_sha256
        {
            return Err(InactiveImportErrorV1::SealedBackupMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InactiveStoreImportReceiptV1 {
    request_id: MigrationDigestV1,
    restore_candidate_id: MigrationDigestV1,
    source_export_bytes_sha256: MigrationDigestV1,
    destination_domain_id: MigrationDigestV1,
    candidate_root_id: MigrationDigestV1,
    pre_import_state_revision: u64,
    post_import_state_revision: u64,
    id: MigrationDigestV1,
    canonical_bytes: Vec<u8>,
}

impl InactiveStoreImportReceiptV1 {
    pub fn from_candidate(
        request: &InactiveStoreImportRequestV1,
        candidate: &RestoreCandidateV1,
        sealed_backup: &[u8],
        pre_import_state_revision: u64,
        post_import_state_revision: u64,
    ) -> Result<Self, InactiveImportErrorV1> {
        request.verify_sealed_backup(sealed_backup)?;
        // The receipt must attest that the candidate was restored from the
        // request's sealed backup, not merely that it names the expected
        // root: bind the candidate's export digest to the backup's own
        // export bytes.
        let backup = SealedBackupV1::decode(sealed_backup)?;
        if MigrationDigestV1::digest_bytes(backup.export().canonical_bytes())?
            != MigrationDigestV1::from_digest(*candidate.source_export_bytes_digest())?
        {
            return Err(InactiveImportErrorV1::CandidateBackupMismatch);
        }
        if candidate.candidate_roots() != [request.expected_candidate_root_id()] {
            return Err(InactiveImportErrorV1::CandidateRootMismatch);
        }
        let restore_candidate_id = MigrationDigestV1::from_digest(candidate.id().into_bytes())?;
        let source_export_bytes_sha256 =
            MigrationDigestV1::from_digest(*candidate.source_export_bytes_digest())?;
        let destination_domain_id =
            MigrationDigestV1::from_digest(candidate.destination_domain_id().into_bytes())?;
        let candidate_root_id =
            MigrationDigestV1::from_digest(request.expected_candidate_root_id().into_bytes())?;
        let value = CborValue::Array(vec![
            request.id().canonical_value(),
            restore_candidate_id.canonical_value(),
            source_export_bytes_sha256.canonical_value(),
            destination_domain_id.canonical_value(),
            candidate_root_id.canonical_value(),
            CborValue::Unsigned(pre_import_state_revision),
            CborValue::Unsigned(post_import_state_revision),
            CborValue::Bool(false),
            CborValue::Bool(false),
        ]);
        let id = MigrationDigestV1::identify(IMPORT_RECEIPT_DOMAIN_V1, &value)?;
        let canonical_bytes = deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(1),
            id.canonical_value(),
            value,
        ]))?;
        Ok(Self {
            request_id: request.id(),
            restore_candidate_id,
            source_export_bytes_sha256,
            destination_domain_id,
            candidate_root_id,
            pre_import_state_revision,
            post_import_state_revision,
            id,
            canonical_bytes,
        })
    }

    pub const fn request_id(&self) -> MigrationDigestV1 {
        self.request_id
    }

    pub const fn restore_candidate_id(&self) -> MigrationDigestV1 {
        self.restore_candidate_id
    }

    pub const fn source_export_bytes_sha256(&self) -> MigrationDigestV1 {
        self.source_export_bytes_sha256
    }

    pub const fn destination_domain_id(&self) -> MigrationDigestV1 {
        self.destination_domain_id
    }

    pub const fn candidate_root_id(&self) -> MigrationDigestV1 {
        self.candidate_root_id
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn pre_import_state_revision(&self) -> u64 {
        self.pre_import_state_revision
    }

    pub const fn post_import_state_revision(&self) -> u64 {
        self.post_import_state_revision
    }

    pub const fn activated(&self) -> bool {
        false
    }

    pub const fn claims_currentness(&self) -> bool {
        false
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InactiveImportErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("inactive import proof sets do not share the same source closure")]
    ProofSetMismatch,
    #[error("inactive import requires a passed before-currentness consumer gate")]
    ConsumerGateNotClosed,
    #[error("sealed backup is empty")]
    EmptySealedBackup,
    #[error("sealed backup byte length overflowed")]
    SealedBackupLengthOverflow,
    #[error("sealed backup bytes do not match the bound import request")]
    SealedBackupMismatch,
    #[error("Store restore candidate roots do not equal the associated expected root")]
    CandidateRootMismatch,
    #[error(transparent)]
    Backup(#[from] ExportError),
    #[error("Store restore candidate was not produced from the bound sealed backup")]
    CandidateBackupMismatch,
}
