use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::ffi::CStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(all(test, unix))]
use std::sync::{Arc, Barrier, Mutex};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::effect_home::{EffectIntentHomeKindV1, HomeTokenV1};
use super::effect_routes::{
    CeremonyRequestModeV1, EffectOriginHomeCompatibilityV1, EffectOriginRouteRoleV1,
};
use super::withdrawal::{
    RemoteClassificationV1, WithdrawalCatalogCellV1, WithdrawalError,
    ceremony_withdrawal_catalog_cell_v1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};
use crate::foundation::core::secure_fs::{CreateIfAbsent, SecureFsError, SecureRoot};

static CEREMONY_INITIALIZATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const CEREMONY_INITIALIZATION_ATTEMPTS: usize = 32;
const ROLLBACK_JOURNAL_CUSTODY_ATTEMPTS: usize = 16;
#[cfg(unix)]
static PROTECTED_CEREMONY_VFS_REGISTRATION: OnceLock<Result<(), i32>> = OnceLock::new();
#[cfg(all(test, unix))]
static PROTECTED_JOURNAL_OPEN_TEST_HOOK: Mutex<Option<ProtectedJournalOpenTestHookV1>> =
    Mutex::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeremonySpecV1 {
    InstallationContextGenesis,
    RepositoryV1Cutover,
    InstallationV1Cutover,
    RecoverRepositoryStoreGeneration,
    RecoverInstallationStoreGeneration,
    ActivateVerifiedRepositoryGeneration,
    ActivateVerifiedInstallationGeneration,
    RecoverPreStoreBinarySlot,
    RecoverPreStoreWriterCohort,
    EstablishRepositoryRecoveryAdmission,
    EstablishInstallationRecoveryAdmission,
}

impl CeremonySpecV1 {
    pub const ALL: [Self; 11] = [
        Self::InstallationContextGenesis,
        Self::RepositoryV1Cutover,
        Self::InstallationV1Cutover,
        Self::RecoverRepositoryStoreGeneration,
        Self::RecoverInstallationStoreGeneration,
        Self::ActivateVerifiedRepositoryGeneration,
        Self::ActivateVerifiedInstallationGeneration,
        Self::RecoverPreStoreBinarySlot,
        Self::RecoverPreStoreWriterCohort,
        Self::EstablishRepositoryRecoveryAdmission,
        Self::EstablishInstallationRecoveryAdmission,
    ];

    pub const fn tag(self) -> u8 {
        match self {
            Self::InstallationContextGenesis => 1,
            Self::RepositoryV1Cutover => 2,
            Self::InstallationV1Cutover => 3,
            Self::RecoverRepositoryStoreGeneration => 4,
            Self::RecoverInstallationStoreGeneration => 5,
            Self::ActivateVerifiedRepositoryGeneration => 6,
            Self::ActivateVerifiedInstallationGeneration => 7,
            Self::RecoverPreStoreBinarySlot => 8,
            Self::RecoverPreStoreWriterCohort => 9,
            Self::EstablishRepositoryRecoveryAdmission => 10,
            Self::EstablishInstallationRecoveryAdmission => 11,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        tag.checked_sub(1)
            .and_then(|index| Self::ALL.get(usize::from(index)))
            .copied()
            .ok_or(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::InstallationContextGenesis => "InstallationContextGenesis",
            Self::RepositoryV1Cutover => "RepositoryV1Cutover",
            Self::InstallationV1Cutover => "InstallationV1Cutover",
            Self::RecoverRepositoryStoreGeneration => "RecoverRepositoryStoreGeneration",
            Self::RecoverInstallationStoreGeneration => "RecoverInstallationStoreGeneration",
            Self::ActivateVerifiedRepositoryGeneration => "ActivateVerifiedRepositoryGeneration",
            Self::ActivateVerifiedInstallationGeneration => {
                "ActivateVerifiedInstallationGeneration"
            }
            Self::RecoverPreStoreBinarySlot => "RecoverPreStoreBinarySlot",
            Self::RecoverPreStoreWriterCohort => "RecoverPreStoreWriterCohort",
            Self::EstablishRepositoryRecoveryAdmission => "EstablishRepositoryRecoveryAdmission",
            Self::EstablishInstallationRecoveryAdmission => {
                "EstablishInstallationRecoveryAdmission"
            }
        }
    }

    pub const fn home_kind(self) -> EffectIntentHomeKindV1 {
        if matches!(self, Self::InstallationContextGenesis) {
            EffectIntentHomeKindV1::NoStoreCeremony
        } else {
            EffectIntentHomeKindV1::PreStoreCeremony
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyOwnerAuthorityV1 {
    owner_basis: HomeTokenV1,
    owner_basis_commitment: HomeTokenV1,
}

impl ProtectedCeremonyOwnerAuthorityV1 {
    pub fn from_owner_basis(
        owner_basis: HomeTokenV1,
    ) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        require_nonzero(owner_basis)?;
        Ok(Self {
            owner_basis,
            owner_basis_commitment: owner_basis_commitment(owner_basis)?,
        })
    }

    pub fn issue_request(
        &self,
        store: &ProtectedCeremonyEffectStoreV1,
        mode: CeremonyRequestModeV1,
        expected_old_token: HomeTokenV1,
        candidate_seal: HomeTokenV1,
        idempotency_key: HomeTokenV1,
    ) -> Result<ProtectedCeremonyEffectRequestV1, ProtectedCeremonyEffectErrorV1> {
        store.issue_owner_request(
            self,
            mode,
            expected_old_token,
            candidate_seal,
            idempotency_key,
        )
    }

    pub fn decode_request(
        &self,
        store: &ProtectedCeremonyEffectStoreV1,
        encoded: &[u8],
    ) -> Result<ProtectedCeremonyEffectRequestV1, ProtectedCeremonyEffectErrorV1> {
        if self.owner_basis_commitment != store.owner_basis_commitment {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        }
        let value = deterministic_cbor::decode(encoded)?;
        let CborValue::Array(fields) = &value else {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        };
        let [
            CborValue::Text(domain),
            request_id_value,
            CborValue::Unsigned(spec_tag),
            CborValue::Unsigned(home_value),
            realm,
            carrier_incarnation,
            authority_basis,
            CborValue::Unsigned(mode_value),
            expected_old_token,
            candidate_seal,
            idempotency_key,
            CborValue::Unsigned(carrier_revision),
            commitment,
        ] = fields.as_slice()
        else {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        };
        if domain != "maestro.vnext.protected-ceremony-effect-request.v1" {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        }
        let authority = ProtectedCeremonyAuthorityV1 {
            spec: CeremonySpecV1::from_tag(
                u8::try_from(*spec_tag)
                    .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier)?,
            )?,
            home: home_from_tag(
                u8::try_from(*home_value)
                    .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier)?,
            )?,
            realm: token_from_cbor(realm)?,
            carrier_incarnation: token_from_cbor(carrier_incarnation)?,
            authority_basis: token_from_cbor(authority_basis)?,
            mode: mode_from_tag(
                u8::try_from(*mode_value)
                    .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier)?,
            )?,
            expected_old_token: token_from_cbor(expected_old_token)?,
            candidate_seal: token_from_cbor(candidate_seal)?,
            idempotency_key: token_from_cbor(idempotency_key)?,
            carrier_revision: *carrier_revision,
            commitment: token_from_cbor(commitment)?,
        };
        let request = ProtectedCeremonyEffectRequestV1 {
            id: token_from_cbor(request_id_value)?,
            authority,
        };
        let expected_commitment = authority_commitment(
            request.authority.spec,
            request.authority.home,
            request.authority.realm,
            request.authority.carrier_incarnation,
            self.owner_basis_commitment,
            request.authority.mode,
            request.authority.expected_old_token,
            request.authority.candidate_seal,
            request.authority.idempotency_key,
            request.authority.carrier_revision,
        )?;
        if request.authority.spec != store.spec
            || request.authority.home != store.spec.home_kind()
            || request.authority.realm != store.realm
            || request.authority.carrier_incarnation != store.incarnation
            || request.authority.authority_basis != self.owner_basis_commitment
            || request.authority.commitment != expected_commitment
            || request.id != request_id(&request.authority)?
            || request.canonical_value()? != value
        {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        }
        Ok(request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedCeremonyEffectPhaseV1 {
    Empty,
    Reserved {
        attempt: HomeTokenV1,
        reservation: HomeTokenV1,
    },
    Sealed {
        attempt: HomeTokenV1,
        seal: HomeTokenV1,
    },
    Resolved {
        attempt: HomeTokenV1,
        result: HomeTokenV1,
    },
    Withdrawn,
}

impl ProtectedCeremonyEffectPhaseV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Empty => 1,
            Self::Reserved { .. } => 2,
            Self::Sealed { .. } => 3,
            Self::Resolved { .. } => 4,
            Self::Withdrawn => 5,
        }
    }

    fn canonical_value(self) -> CborValue {
        match self {
            Self::Empty => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::Reserved {
                attempt,
                reservation,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(attempt.as_bytes()),
                bytes(reservation.as_bytes()),
            ]),
            Self::Sealed { attempt, seal } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                bytes(attempt.as_bytes()),
                bytes(seal.as_bytes()),
            ]),
            Self::Resolved { attempt, result } => CborValue::Array(vec![
                CborValue::Unsigned(4),
                bytes(attempt.as_bytes()),
                bytes(result.as_bytes()),
            ]),
            Self::Withdrawn => CborValue::Array(vec![CborValue::Unsigned(5)]),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyAuthorityV1 {
    spec: CeremonySpecV1,
    home: EffectIntentHomeKindV1,
    realm: HomeTokenV1,
    carrier_incarnation: HomeTokenV1,
    authority_basis: HomeTokenV1,
    mode: CeremonyRequestModeV1,
    expected_old_token: HomeTokenV1,
    candidate_seal: HomeTokenV1,
    idempotency_key: HomeTokenV1,
    carrier_revision: u64,
    commitment: HomeTokenV1,
}

impl ProtectedCeremonyAuthorityV1 {
    pub const fn spec(&self) -> CeremonySpecV1 {
        self.spec
    }

    pub const fn home(&self) -> EffectIntentHomeKindV1 {
        self.home
    }

    pub const fn realm(&self) -> HomeTokenV1 {
        self.realm
    }

    pub const fn carrier_incarnation(&self) -> HomeTokenV1 {
        self.carrier_incarnation
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyEffectRequestV1 {
    id: HomeTokenV1,
    authority: ProtectedCeremonyAuthorityV1,
}

impl ProtectedCeremonyEffectRequestV1 {
    pub const fn id(&self) -> HomeTokenV1 {
        self.id
    }

    pub const fn mode(&self) -> CeremonyRequestModeV1 {
        self.authority.mode
    }

    pub const fn performs_provider_io(&self) -> bool {
        false
    }

    pub const fn creates_effect_records(&self) -> bool {
        !matches!(
            self.authority.mode,
            CeremonyRequestModeV1::ResolveResult | CeremonyRequestModeV1::Withdraw
        )
    }

    pub const fn creates_attempt_or_run(&self) -> bool {
        false
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-effect-request.v1")?,
            bytes(self.id.as_bytes()),
            CborValue::Unsigned(u64::from(self.authority.spec.tag())),
            CborValue::Unsigned(home_tag(self.authority.home)),
            bytes(self.authority.realm.as_bytes()),
            bytes(self.authority.carrier_incarnation.as_bytes()),
            bytes(self.authority.authority_basis.as_bytes()),
            CborValue::Unsigned(mode_tag(self.authority.mode)),
            bytes(self.authority.expected_old_token.as_bytes()),
            bytes(self.authority.candidate_seal.as_bytes()),
            bytes(self.authority.idempotency_key.as_bytes()),
            CborValue::Unsigned(self.authority.carrier_revision),
            bytes(self.authority.commitment.as_bytes()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.canonical_value()?)
    }

    pub fn withdrawal_catalog_cell(
        &self,
        classification: RemoteClassificationV1,
    ) -> Result<WithdrawalCatalogCellV1, WithdrawalError> {
        if self.authority.mode != CeremonyRequestModeV1::Withdraw {
            return Err(WithdrawalError::CatalogCellMismatch);
        }
        ceremony_withdrawal_catalog_cell_v1(
            classification,
            self.authority.spec.tag(),
            self.authority.home,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyEffectCarrierV1 {
    spec: CeremonySpecV1,
    home: EffectIntentHomeKindV1,
    realm: HomeTokenV1,
    incarnation: HomeTokenV1,
    owner_basis_commitment: HomeTokenV1,
    current_token: HomeTokenV1,
    revision: u64,
    phase: ProtectedCeremonyEffectPhaseV1,
}

impl ProtectedCeremonyEffectCarrierV1 {
    fn genesis(
        spec: CeremonySpecV1,
        realm: HomeTokenV1,
        incarnation: HomeTokenV1,
        owner_basis_commitment: HomeTokenV1,
    ) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        require_nonzero(realm)?;
        require_nonzero(incarnation)?;
        require_nonzero(owner_basis_commitment)?;
        let home = spec.home_kind();
        let current_token = token(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-effect-carrier-genesis.v2")?,
            CborValue::Unsigned(u64::from(spec.tag())),
            CborValue::Unsigned(home_tag(home)),
            bytes(realm.as_bytes()),
            bytes(incarnation.as_bytes()),
            bytes(owner_basis_commitment.as_bytes()),
        ]))?;
        Ok(Self {
            spec,
            home,
            realm,
            incarnation,
            owner_basis_commitment,
            current_token,
            revision: 1,
            phase: ProtectedCeremonyEffectPhaseV1::Empty,
        })
    }

    pub const fn current_token(self) -> HomeTokenV1 {
        self.current_token
    }

    pub const fn revision(self) -> u64 {
        self.revision
    }

    pub const fn phase(self) -> ProtectedCeremonyEffectPhaseV1 {
        self.phase
    }

    pub const fn realm(self) -> HomeTokenV1 {
        self.realm
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyEffectStoreV1 {
    managed_root: PathBuf,
    root_identity: ManagedRootIdentityV1,
    database_path: PathBuf,
    leaf_identity: ManagedRootIdentityV1,
    spec: CeremonySpecV1,
    realm: HomeTokenV1,
    incarnation: HomeTokenV1,
    owner_basis_commitment: HomeTokenV1,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyCarrierAnchorV1 {
    root_identity: ManagedRootIdentityV1,
    leaf_identity: ManagedRootIdentityV1,
    spec: CeremonySpecV1,
    realm: HomeTokenV1,
    incarnation: HomeTokenV1,
    owner_basis_commitment: HomeTokenV1,
}

impl ProtectedCeremonyCarrierAnchorV1 {
    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-carrier-anchor.v1")?,
            CborValue::Unsigned(self.root_identity.device),
            CborValue::Unsigned(self.root_identity.inode),
            CborValue::Unsigned(self.leaf_identity.device),
            CborValue::Unsigned(self.leaf_identity.inode),
            CborValue::Unsigned(u64::from(self.spec.tag())),
            bytes(self.realm.as_bytes()),
            bytes(self.incarnation.as_bytes()),
            bytes(self.owner_basis_commitment.as_bytes()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.canonical_value()?)
    }

    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        let value = deterministic_cbor::decode(encoded)?;
        let CborValue::Array(fields) = &value else {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        };
        let [
            CborValue::Text(domain),
            CborValue::Unsigned(root_device),
            CborValue::Unsigned(root_inode),
            CborValue::Unsigned(leaf_device),
            CborValue::Unsigned(leaf_inode),
            CborValue::Unsigned(spec),
            realm,
            incarnation,
            owner_basis_commitment,
        ] = fields.as_slice()
        else {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        };
        if domain != "maestro.vnext.protected-ceremony-carrier-anchor.v1" {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        let anchor = Self {
            root_identity: ManagedRootIdentityV1 {
                device: *root_device,
                inode: *root_inode,
            },
            leaf_identity: ManagedRootIdentityV1 {
                device: *leaf_device,
                inode: *leaf_inode,
            },
            spec: CeremonySpecV1::from_tag(
                u8::try_from(*spec)
                    .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
            )?,
            realm: token_from_cbor(realm)?,
            incarnation: token_from_cbor(incarnation)?,
            owner_basis_commitment: token_from_cbor(owner_basis_commitment)?,
        };
        if anchor.canonical_value()? != value {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        Ok(anchor)
    }
}

impl ProtectedCeremonyEffectStoreV1 {
    pub fn initialize(
        managed_root: impl AsRef<Path>,
        spec: CeremonySpecV1,
        incarnation: HomeTokenV1,
        owner_authority: &ProtectedCeremonyOwnerAuthorityV1,
    ) -> Result<(Self, ProtectedCeremonyCarrierAnchorV1), ProtectedCeremonyEffectErrorV1> {
        require_nonzero(incarnation)?;
        let owner_basis_commitment = owner_authority.owner_basis_commitment;
        let (managed_root, root_identity) = validate_managed_root(managed_root.as_ref())?;
        let database_name = format!(
            "protected-ceremony-{:02}-{}.sqlite3",
            spec.tag(),
            hex(incarnation.as_bytes())
        );
        let database_path = managed_root.join(&database_name);
        validate_database_leaf(&managed_root, &database_path)?;
        let secure_root = SecureRoot::open(&managed_root).map_err(secure_durability_error)?;
        if secure_root
            .bind_optional_regular_file(&database_name)
            .map_err(secure_durability_error)?
            .is_some()
        {
            return recover_published_ceremony_store(
                managed_root,
                root_identity,
                database_path,
                spec,
                incarnation,
                owner_authority,
            );
        }
        let (temporary_name, temporary_path, leaf_identity) =
            create_ceremony_initialization_leaf(&secure_root, &managed_root)?;
        let realm = realm_identity(spec, incarnation, root_identity, leaf_identity)?;
        let mut store = Self {
            managed_root,
            root_identity,
            database_path: temporary_path.clone(),
            leaf_identity,
            spec,
            realm,
            incarnation,
            owner_basis_commitment,
        };
        let anchor = ProtectedCeremonyCarrierAnchorV1 {
            root_identity,
            leaf_identity,
            spec,
            realm,
            incarnation,
            owner_basis_commitment,
        };
        let (mut connection, journal_guard) = store.protected_connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(durability_error)?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS protected_ceremony_effect_carrier (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    spec_tag INTEGER NOT NULL,
                    home_tag INTEGER NOT NULL,
                    realm BLOB NOT NULL CHECK (length(realm) = 32),
                    incarnation BLOB NOT NULL CHECK (length(incarnation) = 32),
                    owner_basis_commitment BLOB NOT NULL CHECK (length(owner_basis_commitment) = 32),
                    current_token BLOB NOT NULL CHECK (length(current_token) = 32),
                    revision INTEGER NOT NULL CHECK (revision > 0),
                    phase_tag INTEGER NOT NULL,
                    attempt BLOB CHECK (attempt IS NULL OR length(attempt) = 32),
                    phase_value BLOB CHECK (phase_value IS NULL OR length(phase_value) = 32)
                ) STRICT;
                CREATE TABLE IF NOT EXISTS protected_ceremony_carrier_anchor (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    root_device BLOB NOT NULL CHECK (length(root_device) = 8),
                    root_inode BLOB NOT NULL CHECK (length(root_inode) = 8),
                    leaf_device BLOB NOT NULL CHECK (length(leaf_device) = 8),
                    leaf_inode BLOB NOT NULL CHECK (length(leaf_inode) = 8),
                    spec_tag INTEGER NOT NULL,
                    realm BLOB NOT NULL CHECK (length(realm) = 32),
                    incarnation BLOB NOT NULL CHECK (length(incarnation) = 32),
                    owner_basis_commitment BLOB NOT NULL CHECK (length(owner_basis_commitment) = 32)
                ) STRICT;
                CREATE TABLE IF NOT EXISTS protected_ceremony_effect_idempotency (
                    idempotency_key BLOB PRIMARY KEY CHECK (length(idempotency_key) = 32),
                    meaning_digest BLOB NOT NULL CHECK (length(meaning_digest) = 32),
                    request_id BLOB NOT NULL CHECK (length(request_id) = 32),
                    prior_token BLOB NOT NULL CHECK (length(prior_token) = 32),
                    authority_commitment BLOB NOT NULL CHECK (length(authority_commitment) = 32),
                    mode_tag INTEGER NOT NULL,
                    candidate_seal BLOB NOT NULL CHECK (length(candidate_seal) = 32),
                    current_token BLOB NOT NULL CHECK (length(current_token) = 32),
                    revision INTEGER NOT NULL CHECK (revision > 0),
                    phase_tag INTEGER NOT NULL,
                    attempt BLOB CHECK (attempt IS NULL OR length(attempt) = 32),
                    phase_value BLOB CHECK (phase_value IS NULL OR length(phase_value) = 32)
                ) STRICT;",
            )
            .map_err(durability_error)?;
        transaction
            .execute(
                "INSERT INTO protected_ceremony_carrier_anchor (
                    singleton, root_device, root_inode, leaf_device, leaf_inode,
                    spec_tag, realm, incarnation, owner_basis_commitment
                 ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    root_identity.device.to_be_bytes().as_slice(),
                    root_identity.inode.to_be_bytes().as_slice(),
                    leaf_identity.device.to_be_bytes().as_slice(),
                    leaf_identity.inode.to_be_bytes().as_slice(),
                    i64::from(spec.tag()),
                    realm.as_bytes().as_slice(),
                    incarnation.as_bytes().as_slice(),
                    owner_basis_commitment.as_bytes().as_slice(),
                ],
            )
            .map_err(durability_error)?;
        let genesis = ProtectedCeremonyEffectCarrierV1::genesis(
            spec,
            realm,
            incarnation,
            owner_basis_commitment,
        )?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO protected_ceremony_effect_carrier (
                    singleton, spec_tag, home_tag, realm, incarnation, owner_basis_commitment,
                    current_token, revision, phase_tag, attempt, phase_value
                 ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, NULL, NULL)",
                params![
                    i64::from(spec.tag()),
                    i64::try_from(home_tag(spec.home_kind()))
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                    realm.as_bytes().as_slice(),
                    incarnation.as_bytes().as_slice(),
                    owner_basis_commitment.as_bytes().as_slice(),
                    genesis.current_token().as_bytes().as_slice(),
                    i64::try_from(genesis.revision())
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                ],
            )
            .map_err(durability_error)?;
        let current = load_protected_carrier(&transaction)?;
        store.validate_identity(current)?;
        if load_protected_anchor(&transaction)? != anchor {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        verify_open_rollback_journal(&transaction, &store.managed_root, &store.database_path)?;
        transaction.commit().map_err(durability_error)?;
        store.verify_live_connection(&connection)?;
        drop(connection);
        drop(journal_guard);
        match secure_root
            .rename_file_no_replace(&temporary_name, &database_name)
            .map_err(secure_durability_error)?
        {
            CreateIfAbsent::Created => {}
            CreateIfAbsent::AlreadyExists => {
                remove_ceremony_initialization_leaf(
                    &secure_root,
                    &temporary_name,
                    &temporary_path,
                    leaf_identity,
                )?;
                return recover_published_ceremony_store(
                    store.managed_root,
                    root_identity,
                    database_path,
                    spec,
                    incarnation,
                    owner_authority,
                );
            }
        }
        verify_database_leaf(&store.managed_root, &database_path, leaf_identity)?;
        store.database_path = database_path;
        let (connection, _journal_guard) = store.protected_connection()?;
        validate_protected_carrier_history(&connection, &store)?;
        Ok((store, anchor))
    }

    pub fn open(
        managed_root: impl AsRef<Path>,
        anchor: &ProtectedCeremonyCarrierAnchorV1,
        owner_authority: &ProtectedCeremonyOwnerAuthorityV1,
    ) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        let (managed_root, root_identity) = validate_managed_root(managed_root.as_ref())?;
        if root_identity != anchor.root_identity
            || owner_authority.owner_basis_commitment != anchor.owner_basis_commitment
        {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        let database_path = managed_root.join(format!(
            "protected-ceremony-{:02}-{}.sqlite3",
            anchor.spec.tag(),
            hex(anchor.incarnation.as_bytes())
        ));
        validate_database_leaf(&managed_root, &database_path)?;
        let leaf_metadata = fs::symlink_metadata(&database_path).map_err(durability_error)?;
        if !leaf_metadata.is_file()
            || !metadata_has_unique_link(&leaf_metadata)
            || metadata_identity(&leaf_metadata) != anchor.leaf_identity
        {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        let realm = realm_identity(
            anchor.spec,
            anchor.incarnation,
            root_identity,
            anchor.leaf_identity,
        )?;
        if realm != anchor.realm {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        let store = Self {
            managed_root,
            root_identity,
            database_path,
            leaf_identity: anchor.leaf_identity,
            spec: anchor.spec,
            realm,
            incarnation: anchor.incarnation,
            owner_basis_commitment: owner_authority.owner_basis_commitment,
        };
        let (connection, _journal_guard) = store.protected_connection()?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(durability_error)?;
        if load_protected_anchor(&transaction)? != *anchor {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        validate_protected_carrier_history(&transaction, &store)?;
        transaction.commit().map_err(durability_error)?;
        store.verify_live_connection(&connection)?;
        Ok(store)
    }

    pub const fn realm(&self) -> HomeTokenV1 {
        self.realm
    }

    pub fn current(
        &self,
    ) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
        let (connection, _journal_guard) = self.protected_connection()?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(durability_error)?;
        let current = validate_protected_carrier_history(&transaction, self)?;
        transaction.commit().map_err(durability_error)?;
        self.verify_live_connection(&connection)?;
        Ok(current)
    }

    fn issue_owner_request(
        &self,
        owner_authority: &ProtectedCeremonyOwnerAuthorityV1,
        mode: CeremonyRequestModeV1,
        expected_old_token: HomeTokenV1,
        candidate_seal: HomeTokenV1,
        idempotency_key: HomeTokenV1,
    ) -> Result<ProtectedCeremonyEffectRequestV1, ProtectedCeremonyEffectErrorV1> {
        require_nonzero(expected_old_token)?;
        require_nonzero(candidate_seal)?;
        require_nonzero(idempotency_key)?;
        if owner_basis_commitment(owner_authority.owner_basis)? != self.owner_basis_commitment {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
        }
        let carrier = self.current()?;
        if carrier.current_token != expected_old_token {
            return Err(ProtectedCeremonyEffectErrorV1::StaleExpectedCarrier);
        }
        validate_mode_for_phase(mode, carrier.phase)?;
        EffectOriginHomeCompatibilityV1::validate(
            ceremony_role(mode),
            carrier.home,
            None,
            Some(mode),
            matches!(self.spec, CeremonySpecV1::InstallationContextGenesis),
        )?;
        let commitment = authority_commitment(
            self.spec,
            carrier.home,
            self.realm,
            self.incarnation,
            self.owner_basis_commitment,
            mode,
            expected_old_token,
            candidate_seal,
            idempotency_key,
            carrier.revision,
        )?;
        let authority = ProtectedCeremonyAuthorityV1 {
            spec: self.spec,
            home: carrier.home,
            realm: self.realm,
            carrier_incarnation: self.incarnation,
            authority_basis: self.owner_basis_commitment,
            mode,
            expected_old_token,
            candidate_seal,
            idempotency_key,
            carrier_revision: carrier.revision,
            commitment,
        };
        let id = request_id(&authority)?;
        Ok(ProtectedCeremonyEffectRequestV1 { id, authority })
    }

    pub fn publish(
        &self,
        request: ProtectedCeremonyEffectRequestV1,
    ) -> Result<ProtectedCeremonyEffectOutcomeV1, ProtectedCeremonyEffectErrorV1> {
        let (mut connection, _journal_guard) = self.protected_connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(durability_error)?;
        validate_protected_carrier_history(&transaction, self)?;
        let meaning = request_meaning(&request.authority)?;
        if let Some(replay) = load_idempotency_outcome(
            &transaction,
            request.authority.idempotency_key,
            meaning,
            request.id,
        )? {
            transaction.commit().map_err(durability_error)?;
            self.verify_live_connection(&connection)?;
            return Ok(replay);
        }
        let current = self.validate_identity(load_protected_carrier(&transaction)?)?;
        validate_request(self, current, &request)?;
        let next_phase = next_phase(current.phase, &request)?;
        let next_revision = current
            .revision
            .checked_add(1)
            .ok_or(ProtectedCeremonyEffectErrorV1::RevisionOverflow)?;
        let next_token = token(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-effect-carrier.v2")?,
            bytes(current.current_token.as_bytes()),
            bytes(request.id.as_bytes()),
            CborValue::Unsigned(next_revision),
            next_phase.canonical_value(),
            bytes(request.authority.commitment.as_bytes()),
        ]))?;
        let (attempt, phase_value) = phase_columns(next_phase);
        let changed = transaction
            .execute(
                "UPDATE protected_ceremony_effect_carrier
                 SET current_token = ?1, revision = ?2, phase_tag = ?3,
                     attempt = ?4, phase_value = ?5
                 WHERE singleton = 1 AND current_token = ?6 AND revision = ?7",
                params![
                    next_token.as_bytes().as_slice(),
                    i64::try_from(next_revision)
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                    i64::from(next_phase.tag()),
                    attempt.map(|value| value.as_bytes().to_vec()),
                    phase_value.map(|value| value.as_bytes().to_vec()),
                    current.current_token.as_bytes().as_slice(),
                    i64::try_from(current.revision)
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                ],
            )
            .map_err(durability_error)?;
        if changed != 1 {
            return Err(ProtectedCeremonyEffectErrorV1::StaleExpectedCarrier);
        }
        transaction
            .execute(
                "INSERT INTO protected_ceremony_effect_idempotency (
                    idempotency_key, meaning_digest, request_id, prior_token,
                    authority_commitment, mode_tag, candidate_seal, current_token,
                    revision, phase_tag, attempt, phase_value
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    request.authority.idempotency_key.as_bytes().as_slice(),
                    meaning.as_slice(),
                    request.id.as_bytes().as_slice(),
                    current.current_token.as_bytes().as_slice(),
                    request.authority.commitment.as_bytes().as_slice(),
                    i64::try_from(mode_tag(request.authority.mode))
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                    request.authority.candidate_seal.as_bytes().as_slice(),
                    next_token.as_bytes().as_slice(),
                    i64::try_from(next_revision)
                        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
                    i64::from(next_phase.tag()),
                    attempt.map(|value| value.as_bytes().to_vec()),
                    phase_value.map(|value| value.as_bytes().to_vec()),
                ],
            )
            .map_err(durability_error)?;
        verify_open_rollback_journal(&transaction, &self.managed_root, &self.database_path)?;
        transaction.commit().map_err(durability_error)?;
        self.verify_live_connection(&connection)?;
        Ok(ProtectedCeremonyEffectOutcomeV1 {
            current_token: next_token,
            revision: next_revision,
            phase: next_phase,
            replayed: false,
            provider_io_operations: 0,
        })
    }

    fn protected_connection(
        &self,
    ) -> Result<(Connection, ProtectedJournalOpenExpectationGuardV1), ProtectedCeremonyEffectErrorV1>
    {
        verify_managed_root(&self.managed_root, self.root_identity)?;
        verify_database_leaf(&self.managed_root, &self.database_path, self.leaf_identity)?;
        let journal_identity = rollback_journal_identity(&self.managed_root, &self.database_path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let (connection, journal_guard) =
            open_protected_ceremony_connection(&self.database_path, flags, journal_identity)?;
        connection
            .busy_timeout(Duration::from_secs(10))
            .map_err(durability_error)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE;")
            .map_err(durability_error)?;
        verify_rollback_journal_custody(&self.managed_root, &self.database_path)?;
        self.verify_live_connection(&connection)?;
        Ok((connection, journal_guard))
    }

    fn verify_live_connection(
        &self,
        connection: &Connection,
    ) -> Result<(), ProtectedCeremonyEffectErrorV1> {
        verify_managed_root(&self.managed_root, self.root_identity)?;
        verify_database_leaf(&self.managed_root, &self.database_path, self.leaf_identity)?;
        verify_rollback_journal_custody(&self.managed_root, &self.database_path)?;
        verify_connection_leaf(connection, &self.database_path, self.leaf_identity)
    }

    fn validate_identity(
        &self,
        carrier: ProtectedCeremonyEffectCarrierV1,
    ) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
        if carrier.spec != self.spec
            || carrier.home != self.spec.home_kind()
            || carrier.realm != self.realm
            || carrier.incarnation != self.incarnation
            || carrier.owner_basis_commitment != self.owner_basis_commitment
        {
            return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
        }
        Ok(carrier)
    }
}

fn recover_published_ceremony_store(
    managed_root: PathBuf,
    root_identity: ManagedRootIdentityV1,
    database_path: PathBuf,
    spec: CeremonySpecV1,
    incarnation: HomeTokenV1,
    owner_authority: &ProtectedCeremonyOwnerAuthorityV1,
) -> Result<
    (
        ProtectedCeremonyEffectStoreV1,
        ProtectedCeremonyCarrierAnchorV1,
    ),
    ProtectedCeremonyEffectErrorV1,
> {
    validate_database_leaf(&managed_root, &database_path)?;
    let metadata = fs::symlink_metadata(&database_path).map_err(durability_error)?;
    if !metadata.is_file() || !metadata_has_unique_link(&metadata) {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    let leaf_identity = metadata_identity(&metadata);
    let realm = realm_identity(spec, incarnation, root_identity, leaf_identity)?;
    let store = ProtectedCeremonyEffectStoreV1 {
        managed_root,
        root_identity,
        database_path,
        leaf_identity,
        spec,
        realm,
        incarnation,
        owner_basis_commitment: owner_authority.owner_basis_commitment,
    };
    let expected_anchor = ProtectedCeremonyCarrierAnchorV1 {
        root_identity,
        leaf_identity,
        spec,
        realm,
        incarnation,
        owner_basis_commitment: owner_authority.owner_basis_commitment,
    };
    let (connection, _journal_guard) = store.protected_connection()?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(durability_error)?;
    if load_protected_anchor(&transaction)? != expected_anchor {
        return Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch);
    }
    validate_protected_carrier_history(&transaction, &store)?;
    transaction.commit().map_err(durability_error)?;
    store.verify_live_connection(&connection)?;
    Ok((store, expected_anchor))
}

fn remove_ceremony_initialization_leaf(
    secure_root: &SecureRoot,
    temporary_name: &str,
    temporary_path: &Path,
    expected_identity: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    verify_database_leaf(
        temporary_path
            .parent()
            .ok_or(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)?,
        temporary_path,
        expected_identity,
    )?;
    let bytes = secure_root
        .read_immutable(temporary_name)
        .map_err(secure_durability_error)?;
    if !secure_root
        .remove_file_if_matches(temporary_name, &bytes)
        .map_err(secure_durability_error)?
    {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCeremonyEffectOutcomeV1 {
    current_token: HomeTokenV1,
    revision: u64,
    phase: ProtectedCeremonyEffectPhaseV1,
    replayed: bool,
    provider_io_operations: u8,
}

impl ProtectedCeremonyEffectOutcomeV1 {
    pub const fn current_token(self) -> HomeTokenV1 {
        self.current_token
    }

    pub const fn revision(self) -> u64 {
        self.revision
    }

    pub const fn phase(self) -> ProtectedCeremonyEffectPhaseV1 {
        self.phase
    }

    pub const fn replayed(self) -> bool {
        self.replayed
    }

    pub const fn provider_io_operations(self) -> u8 {
        self.provider_io_operations
    }
}

fn validate_request(
    store: &ProtectedCeremonyEffectStoreV1,
    current: ProtectedCeremonyEffectCarrierV1,
    request: &ProtectedCeremonyEffectRequestV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    let authority = &request.authority;
    if authority.spec != store.spec
        || authority.home != store.spec.home_kind()
        || authority.realm != store.realm
        || authority.carrier_incarnation != store.incarnation
        || authority.authority_basis != store.owner_basis_commitment
        || authority.expected_old_token != current.current_token
        || authority.carrier_revision != current.revision
        || authority.commitment
            != authority_commitment(
                authority.spec,
                authority.home,
                authority.realm,
                authority.carrier_incarnation,
                authority.authority_basis,
                authority.mode,
                authority.expected_old_token,
                authority.candidate_seal,
                authority.idempotency_key,
                authority.carrier_revision,
            )?
        || request.id != request_id(authority)?
    {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier);
    }
    validate_mode_for_phase(authority.mode, current.phase)
}

fn validate_mode_for_phase(
    mode: CeremonyRequestModeV1,
    phase: ProtectedCeremonyEffectPhaseV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    let legal = matches!(
        (mode, phase),
        (
            CeremonyRequestModeV1::Initiate,
            ProtectedCeremonyEffectPhaseV1::Empty
        ) | (
            CeremonyRequestModeV1::RecoverReserved,
            ProtectedCeremonyEffectPhaseV1::Reserved { .. }
        ) | (
            CeremonyRequestModeV1::ResolveResult,
            ProtectedCeremonyEffectPhaseV1::Sealed { .. }
        ) | (
            CeremonyRequestModeV1::Withdraw,
            ProtectedCeremonyEffectPhaseV1::Empty
        )
    );
    if legal {
        Ok(())
    } else {
        Err(ProtectedCeremonyEffectErrorV1::IllegalLifecycleTransition)
    }
}

fn next_phase(
    phase: ProtectedCeremonyEffectPhaseV1,
    request: &ProtectedCeremonyEffectRequestV1,
) -> Result<ProtectedCeremonyEffectPhaseV1, ProtectedCeremonyEffectErrorV1> {
    validate_mode_for_phase(request.authority.mode, phase)?;
    Ok(match (request.authority.mode, phase) {
        (CeremonyRequestModeV1::Initiate, ProtectedCeremonyEffectPhaseV1::Empty) => {
            ProtectedCeremonyEffectPhaseV1::Reserved {
                attempt: token(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.protected-ceremony-attempt.v1")?,
                    bytes(request.id.as_bytes()),
                    bytes(request.authority.realm.as_bytes()),
                ]))?,
                reservation: request.authority.candidate_seal,
            }
        }
        (
            CeremonyRequestModeV1::RecoverReserved,
            ProtectedCeremonyEffectPhaseV1::Reserved { attempt, .. },
        ) => ProtectedCeremonyEffectPhaseV1::Sealed {
            attempt,
            seal: request.authority.candidate_seal,
        },
        (
            CeremonyRequestModeV1::ResolveResult,
            ProtectedCeremonyEffectPhaseV1::Sealed { attempt, .. },
        ) => ProtectedCeremonyEffectPhaseV1::Resolved {
            attempt,
            result: request.authority.candidate_seal,
        },
        (CeremonyRequestModeV1::Withdraw, ProtectedCeremonyEffectPhaseV1::Empty) => {
            ProtectedCeremonyEffectPhaseV1::Withdrawn
        }
        _ => return Err(ProtectedCeremonyEffectErrorV1::IllegalLifecycleTransition),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the Ceremony authority commitment binds every exact owner, carrier, mode, replay, and currentness dimension"
)]
fn authority_commitment(
    spec: CeremonySpecV1,
    home: EffectIntentHomeKindV1,
    realm: HomeTokenV1,
    incarnation: HomeTokenV1,
    owner_basis_commitment: HomeTokenV1,
    mode: CeremonyRequestModeV1,
    expected_old_token: HomeTokenV1,
    candidate_seal: HomeTokenV1,
    idempotency_key: HomeTokenV1,
    carrier_revision: u64,
) -> Result<HomeTokenV1, CborError> {
    token(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-ceremony-authority.v1")?,
        CborValue::Unsigned(u64::from(spec.tag())),
        CborValue::Unsigned(home_tag(home)),
        bytes(realm.as_bytes()),
        bytes(incarnation.as_bytes()),
        bytes(owner_basis_commitment.as_bytes()),
        CborValue::Unsigned(mode_tag(mode)),
        bytes(expected_old_token.as_bytes()),
        bytes(candidate_seal.as_bytes()),
        bytes(idempotency_key.as_bytes()),
        CborValue::Unsigned(carrier_revision),
    ]))
}

fn owner_basis_commitment(owner_basis: HomeTokenV1) -> Result<HomeTokenV1, CborError> {
    token(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-ceremony-owner-basis-commitment.v1")?,
        bytes(owner_basis.as_bytes()),
    ]))
}

fn request_id(authority: &ProtectedCeremonyAuthorityV1) -> Result<HomeTokenV1, CborError> {
    token(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-ceremony-effect-request.v2")?,
        bytes(authority.commitment.as_bytes()),
        bytes(authority.idempotency_key.as_bytes()),
    ]))
}

fn request_meaning(authority: &ProtectedCeremonyAuthorityV1) -> Result<[u8; 32], CborError> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-effect-meaning.v1")?,
            bytes(authority.commitment.as_bytes()),
        ]))?)
        .into(),
    )
}

fn load_idempotency_outcome(
    transaction: &Transaction<'_>,
    key: HomeTokenV1,
    expected_meaning: [u8; 32],
    expected_request: HomeTokenV1,
) -> Result<Option<ProtectedCeremonyEffectOutcomeV1>, ProtectedCeremonyEffectErrorV1> {
    let row = transaction
        .query_row(
            "SELECT meaning_digest, request_id, current_token, revision, phase_tag,
                    attempt, phase_value
             FROM protected_ceremony_effect_idempotency WHERE idempotency_key = ?1",
            params![key.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<Vec<u8>>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(durability_error)?;
    let Some((meaning, request, current_token, revision, phase_tag, attempt, phase_value)) = row
    else {
        return Ok(None);
    };
    if meaning.as_slice() != expected_meaning || token_from_bytes(&request)? != expected_request {
        return Err(ProtectedCeremonyEffectErrorV1::IdempotencyMeaningConflict);
    }
    let revision = u64::try_from(revision)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
    let phase = phase_from_columns(
        u8::try_from(phase_tag)
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
        attempt.as_deref().map(token_from_bytes).transpose()?,
        phase_value.as_deref().map(token_from_bytes).transpose()?,
    )?;
    Ok(Some(ProtectedCeremonyEffectOutcomeV1 {
        current_token: token_from_bytes(&current_token)?,
        revision,
        phase,
        replayed: true,
        provider_io_operations: 0,
    }))
}

fn ceremony_role(mode: CeremonyRequestModeV1) -> EffectOriginRouteRoleV1 {
    match mode {
        CeremonyRequestModeV1::Initiate => EffectOriginRouteRoleV1::CeremonyInitiate,
        CeremonyRequestModeV1::RecoverReserved => EffectOriginRouteRoleV1::CeremonyRecoverReserved,
        CeremonyRequestModeV1::ResolveResult => EffectOriginRouteRoleV1::CeremonyResolveResult,
        CeremonyRequestModeV1::Withdraw => EffectOriginRouteRoleV1::CeremonyWithdraw,
    }
}

const fn mode_tag(mode: CeremonyRequestModeV1) -> u64 {
    match mode {
        CeremonyRequestModeV1::Initiate => 1,
        CeremonyRequestModeV1::RecoverReserved => 2,
        CeremonyRequestModeV1::ResolveResult => 3,
        CeremonyRequestModeV1::Withdraw => 4,
    }
}

fn mode_from_tag(tag: u8) -> Result<CeremonyRequestModeV1, ProtectedCeremonyEffectErrorV1> {
    match tag {
        1 => Ok(CeremonyRequestModeV1::Initiate),
        2 => Ok(CeremonyRequestModeV1::RecoverReserved),
        3 => Ok(CeremonyRequestModeV1::ResolveResult),
        4 => Ok(CeremonyRequestModeV1::Withdraw),
        _ => Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier),
    }
}

const fn home_tag(home: EffectIntentHomeKindV1) -> u64 {
    match home {
        EffectIntentHomeKindV1::ActiveStore => 1,
        EffectIntentHomeKindV1::NoStoreCeremony => 2,
        EffectIntentHomeKindV1::PreStoreCeremony => 3,
    }
}

fn home_from_tag(tag: u8) -> Result<EffectIntentHomeKindV1, ProtectedCeremonyEffectErrorV1> {
    match tag {
        1 => Ok(EffectIntentHomeKindV1::ActiveStore),
        2 => Ok(EffectIntentHomeKindV1::NoStoreCeremony),
        3 => Ok(EffectIntentHomeKindV1::PreStoreCeremony),
        _ => Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier),
    }
}

fn phase_columns(
    phase: ProtectedCeremonyEffectPhaseV1,
) -> (Option<HomeTokenV1>, Option<HomeTokenV1>) {
    match phase {
        ProtectedCeremonyEffectPhaseV1::Empty | ProtectedCeremonyEffectPhaseV1::Withdrawn => {
            (None, None)
        }
        ProtectedCeremonyEffectPhaseV1::Reserved {
            attempt,
            reservation,
        } => (Some(attempt), Some(reservation)),
        ProtectedCeremonyEffectPhaseV1::Sealed { attempt, seal } => (Some(attempt), Some(seal)),
        ProtectedCeremonyEffectPhaseV1::Resolved { attempt, result } => {
            (Some(attempt), Some(result))
        }
    }
}

fn phase_from_columns(
    tag: u8,
    attempt: Option<HomeTokenV1>,
    value: Option<HomeTokenV1>,
) -> Result<ProtectedCeremonyEffectPhaseV1, ProtectedCeremonyEffectErrorV1> {
    match (tag, attempt, value) {
        (1, None, None) => Ok(ProtectedCeremonyEffectPhaseV1::Empty),
        (2, Some(attempt), Some(reservation)) => Ok(ProtectedCeremonyEffectPhaseV1::Reserved {
            attempt,
            reservation,
        }),
        (3, Some(attempt), Some(seal)) => {
            Ok(ProtectedCeremonyEffectPhaseV1::Sealed { attempt, seal })
        }
        (4, Some(attempt), Some(result)) => {
            Ok(ProtectedCeremonyEffectPhaseV1::Resolved { attempt, result })
        }
        (5, None, None) => Ok(ProtectedCeremonyEffectPhaseV1::Withdrawn),
        _ => Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier),
    }
}

fn load_protected_carrier(
    transaction: &Transaction<'_>,
) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
    load_protected_carrier_row(transaction)
}

fn load_protected_anchor(
    transaction: &Transaction<'_>,
) -> Result<ProtectedCeremonyCarrierAnchorV1, ProtectedCeremonyEffectErrorV1> {
    load_protected_anchor_from_connection(transaction)
}

fn load_protected_anchor_from_connection(
    connection: &Connection,
) -> Result<ProtectedCeremonyCarrierAnchorV1, ProtectedCeremonyEffectErrorV1> {
    let row = connection
        .query_row(
            "SELECT root_device, root_inode, leaf_device, leaf_inode,
                    spec_tag, realm, incarnation, owner_basis_commitment
             FROM protected_ceremony_carrier_anchor WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                    row.get::<_, Vec<u8>>(7)?,
                ))
            },
        )
        .map_err(durability_error)?;
    let tag = u8::try_from(row.4)
        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
    Ok(ProtectedCeremonyCarrierAnchorV1 {
        root_identity: ManagedRootIdentityV1 {
            device: exact_u64(&row.0)?,
            inode: exact_u64(&row.1)?,
        },
        leaf_identity: ManagedRootIdentityV1 {
            device: exact_u64(&row.2)?,
            inode: exact_u64(&row.3)?,
        },
        spec: CeremonySpecV1::from_tag(tag)?,
        realm: token_from_bytes(&row.5)?,
        incarnation: token_from_bytes(&row.6)?,
        owner_basis_commitment: token_from_bytes(&row.7)?,
    })
}

fn load_protected_carrier_from_connection(
    connection: &Connection,
) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
    load_protected_carrier_row(connection)
}

fn validate_protected_carrier_history(
    connection: &Connection,
    store: &ProtectedCeremonyEffectStoreV1,
) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
    let mut previous = ProtectedCeremonyEffectCarrierV1::genesis(
        store.spec,
        store.realm,
        store.incarnation,
        store.owner_basis_commitment,
    )?;
    let mut expected_revision = 2_u64;
    let mut statement = connection
        .prepare(
            "SELECT idempotency_key, meaning_digest, request_id, prior_token,
                    authority_commitment, mode_tag, candidate_seal, current_token,
                    revision, phase_tag, attempt, phase_value
             FROM protected_ceremony_effect_idempotency
             ORDER BY revision ASC",
        )
        .map_err(durability_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, Vec<u8>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<Vec<u8>>>(10)?,
                row.get::<_, Option<Vec<u8>>>(11)?,
            ))
        })
        .map_err(durability_error)?;
    for row in rows {
        let (
            idempotency_key,
            meaning_digest,
            stored_request_id,
            stored_prior_token,
            stored_authority_commitment,
            stored_mode_tag,
            candidate_seal,
            stored_current_token,
            stored_revision,
            stored_phase_tag,
            stored_attempt,
            stored_phase_value,
        ) = row.map_err(durability_error)?;
        let revision = u64::try_from(stored_revision)
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
        if revision != expected_revision
            || token_from_bytes(&stored_prior_token)? != previous.current_token
        {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        let mode = mode_from_tag(
            u8::try_from(stored_mode_tag)
                .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
        )?;
        let idempotency_key = token_from_bytes(&idempotency_key)?;
        let candidate_seal = token_from_bytes(&candidate_seal)?;
        let expected_commitment = authority_commitment(
            store.spec,
            store.spec.home_kind(),
            store.realm,
            store.incarnation,
            store.owner_basis_commitment,
            mode,
            previous.current_token,
            candidate_seal,
            idempotency_key,
            previous.revision,
        )?;
        if token_from_bytes(&stored_authority_commitment)? != expected_commitment {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        let authority = ProtectedCeremonyAuthorityV1 {
            spec: store.spec,
            home: store.spec.home_kind(),
            realm: store.realm,
            carrier_incarnation: store.incarnation,
            authority_basis: store.owner_basis_commitment,
            mode,
            expected_old_token: previous.current_token,
            candidate_seal,
            idempotency_key,
            carrier_revision: previous.revision,
            commitment: expected_commitment,
        };
        let expected_request_id = request_id(&authority)?;
        if token_from_bytes(&stored_request_id)? != expected_request_id
            || meaning_digest.as_slice() != request_meaning(&authority)?
        {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        let request = ProtectedCeremonyEffectRequestV1 {
            id: expected_request_id,
            authority,
        };
        let phase = phase_from_columns(
            u8::try_from(stored_phase_tag)
                .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
            stored_attempt
                .as_deref()
                .map(token_from_bytes)
                .transpose()?,
            stored_phase_value
                .as_deref()
                .map(token_from_bytes)
                .transpose()?,
        )?;
        let expected_phase = next_phase(previous.phase, &request)?;
        let expected_token = token(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-ceremony-effect-carrier.v2")?,
            bytes(previous.current_token.as_bytes()),
            bytes(expected_request_id.as_bytes()),
            CborValue::Unsigned(revision),
            expected_phase.canonical_value(),
            bytes(expected_commitment.as_bytes()),
        ]))?;
        if phase != expected_phase || token_from_bytes(&stored_current_token)? != expected_token {
            return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
        }
        previous = ProtectedCeremonyEffectCarrierV1 {
            spec: store.spec,
            home: store.spec.home_kind(),
            realm: store.realm,
            incarnation: store.incarnation,
            owner_basis_commitment: store.owner_basis_commitment,
            current_token: expected_token,
            revision,
            phase,
        };
        expected_revision = expected_revision
            .checked_add(1)
            .ok_or(ProtectedCeremonyEffectErrorV1::RevisionOverflow)?;
    }
    drop(statement);
    let current = store.validate_identity(load_protected_carrier_from_connection(connection)?)?;
    if current != previous {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    Ok(current)
}

fn load_protected_carrier_row(
    connection: &Connection,
) -> Result<ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1> {
    let row = connection
        .query_row(
            "SELECT spec_tag, home_tag, realm, incarnation, owner_basis_commitment, current_token,
                    revision, phase_tag, attempt, phase_value
             FROM protected_ceremony_effect_carrier WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<Vec<u8>>>(8)?,
                    row.get::<_, Option<Vec<u8>>>(9)?,
                ))
            },
        )
        .map_err(durability_error)?;
    let spec = CeremonySpecV1::from_tag(
        u8::try_from(row.0)
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
    )?;
    let home = home_from_tag(
        u8::try_from(row.1)
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
    )?;
    let realm = token_from_bytes(&row.2)?;
    let incarnation = token_from_bytes(&row.3)?;
    let owner_basis_commitment = token_from_bytes(&row.4)?;
    let current_token = token_from_bytes(&row.5)?;
    let revision = u64::try_from(row.6)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
    let phase = phase_from_columns(
        u8::try_from(row.7)
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
        row.8.as_deref().map(token_from_bytes).transpose()?,
        row.9.as_deref().map(token_from_bytes).transpose()?,
    )?;
    if home != spec.home_kind() {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    Ok(ProtectedCeremonyEffectCarrierV1 {
        spec,
        home,
        realm,
        incarnation,
        owner_basis_commitment,
        current_token,
        revision,
        phase,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ManagedRootIdentityV1 {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct ProtectedJournalOpenExpectationV1 {
    path: PathBuf,
    existing_identity: Option<ManagedRootIdentityV1>,
}

#[cfg(unix)]
thread_local! {
    static PROTECTED_JOURNAL_OPEN_EXPECTATION: RefCell<Option<ProtectedJournalOpenExpectationV1>> = const { RefCell::new(None) };
}

#[cfg(unix)]
struct ProtectedJournalOpenExpectationGuardV1;

#[cfg(unix)]
impl ProtectedJournalOpenExpectationGuardV1 {
    fn arm(
        path: PathBuf,
        existing_identity: Option<ManagedRootIdentityV1>,
    ) -> Result<Self, ProtectedCeremonyEffectErrorV1> {
        let armed = PROTECTED_JOURNAL_OPEN_EXPECTATION.with(|expectation| {
            let mut expectation = expectation.borrow_mut();
            if expectation.is_some() {
                return false;
            }
            *expectation = Some(ProtectedJournalOpenExpectationV1 {
                path,
                existing_identity,
            });
            true
        });
        if !armed {
            return Err(ProtectedCeremonyEffectErrorV1::Durability(
                "protected ceremony journal expectation is already armed".to_owned(),
            ));
        }
        Ok(Self)
    }
}

#[cfg(unix)]
impl Drop for ProtectedJournalOpenExpectationGuardV1 {
    fn drop(&mut self) {
        PROTECTED_JOURNAL_OPEN_EXPECTATION.with(|expectation| {
            *expectation.borrow_mut() = None;
        });
    }
}

#[cfg(not(unix))]
struct ProtectedJournalOpenExpectationGuardV1;

#[cfg(all(test, unix))]
#[derive(Clone)]
struct ProtectedJournalOpenTestHookV1 {
    path: PathBuf,
    reached_open: Arc<Barrier>,
    continue_open: Arc<Barrier>,
}

#[cfg(all(test, unix))]
fn wait_at_protected_journal_open_test_hook(name: *const std::ffi::c_char) {
    if name.is_null() {
        return;
    }
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(unsafe {
        CStr::from_ptr(name).to_bytes()
    }));
    let hook = PROTECTED_JOURNAL_OPEN_TEST_HOOK
        .lock()
        .expect("invariant: protected journal test hook mutex is not poisoned")
        .as_ref()
        .filter(|hook| hook.path == path)
        .cloned();
    if let Some(hook) = hook {
        hook.reached_open.wait();
        hook.continue_open.wait();
    }
}

#[cfg(unix)]
unsafe extern "C" fn protected_ceremony_vfs_open(
    vfs: *mut rusqlite::ffi::sqlite3_vfs,
    name: rusqlite::ffi::sqlite3_filename,
    file: *mut rusqlite::ffi::sqlite3_file,
    flags: i32,
    output_flags: *mut i32,
) -> i32 {
    let open_result = std::panic::catch_unwind(|| unsafe {
        if vfs.is_null() {
            return rusqlite::ffi::SQLITE_CANTOPEN;
        }
        let delegated_vfs = (*vfs).pAppData.cast::<rusqlite::ffi::sqlite3_vfs>();
        if delegated_vfs.is_null() {
            return rusqlite::ffi::SQLITE_CANTOPEN;
        }
        let Some(delegated_open) = (*delegated_vfs).xOpen else {
            return rusqlite::ffi::SQLITE_CANTOPEN;
        };
        let is_main_journal = flags & rusqlite::ffi::SQLITE_OPEN_MAIN_JOURNAL != 0;
        let mut hardened_flags = flags;
        if is_main_journal {
            hardened_flags |= rusqlite::ffi::SQLITE_OPEN_NOFOLLOW;
            if flags & rusqlite::ffi::SQLITE_OPEN_CREATE != 0 {
                hardened_flags |= rusqlite::ffi::SQLITE_OPEN_EXCLUSIVE;
            }
            #[cfg(test)]
            wait_at_protected_journal_open_test_hook(name);
        }
        let result = delegated_open(delegated_vfs, name, file, hardened_flags, output_flags);
        if result != rusqlite::ffi::SQLITE_OK || !is_main_journal {
            return result;
        }
        if protected_journal_open_matches_expectation(name) {
            return result;
        }
        if !file.is_null()
            && !(*file).pMethods.is_null()
            && let Some(close) = (*(*file).pMethods).xClose
        {
            let _ = close(file);
        }
        rusqlite::ffi::SQLITE_CANTOPEN
    });
    open_result.unwrap_or(rusqlite::ffi::SQLITE_CANTOPEN)
}

#[cfg(unix)]
fn protected_journal_open_matches_expectation(name: *const std::ffi::c_char) -> bool {
    if name.is_null() {
        return false;
    }
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(unsafe {
        CStr::from_ptr(name).to_bytes()
    }));
    PROTECTED_JOURNAL_OPEN_EXPECTATION.with(|expectation| {
        let expectation = expectation.borrow();
        let Some(expectation) = expectation.as_ref() else {
            return false;
        };
        if path != expectation.path {
            return false;
        }
        let Some(parent) = path.parent() else {
            return false;
        };
        let Some(name) = path.file_name() else {
            return false;
        };
        let Ok(root) = SecureRoot::open(parent) else {
            return false;
        };
        if root.bind_regular_file(name).is_err() {
            return false;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            return false;
        };
        expectation
            .existing_identity
            .is_none_or(|identity| metadata_identity(&metadata) == identity)
    })
}

#[cfg(unix)]
fn protected_ceremony_vfs_name() -> Result<&'static CStr, ProtectedCeremonyEffectErrorV1> {
    let registration = PROTECTED_CEREMONY_VFS_REGISTRATION.get_or_init(|| unsafe {
        if rusqlite::ffi::sqlite3_initialize() != rusqlite::ffi::SQLITE_OK {
            return Err(rusqlite::ffi::SQLITE_ERROR);
        }
        let delegated_vfs = rusqlite::ffi::sqlite3_vfs_find(std::ptr::null());
        if delegated_vfs.is_null() {
            return Err(rusqlite::ffi::SQLITE_NOTFOUND);
        }
        let mut protected_vfs = Box::new(*delegated_vfs);
        protected_vfs.pNext = std::ptr::null_mut();
        protected_vfs.zName = c"maestro-protected-ceremony-v1".as_ptr();
        protected_vfs.pAppData = delegated_vfs.cast();
        protected_vfs.xOpen = Some(protected_ceremony_vfs_open);
        let protected_vfs = Box::into_raw(protected_vfs);
        let result = rusqlite::ffi::sqlite3_vfs_register(protected_vfs, 0);
        if result == rusqlite::ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(result)
        }
    });
    match registration {
        Ok(()) => Ok(c"maestro-protected-ceremony-v1"),
        Err(code) => Err(ProtectedCeremonyEffectErrorV1::Durability(format!(
            "protected ceremony VFS registration failed: sqlite code {code}"
        ))),
    }
}

fn rollback_journal_path(database: &Path) -> PathBuf {
    let mut path = database.as_os_str().to_os_string();
    path.push("-journal");
    PathBuf::from(path)
}

#[cfg(unix)]
fn open_protected_ceremony_connection(
    database: &Path,
    flags: OpenFlags,
    existing_journal_identity: Option<ManagedRootIdentityV1>,
) -> Result<(Connection, ProtectedJournalOpenExpectationGuardV1), ProtectedCeremonyEffectErrorV1> {
    let vfs = protected_ceremony_vfs_name()?;
    let guard = ProtectedJournalOpenExpectationGuardV1::arm(
        rollback_journal_path(database),
        existing_journal_identity,
    )?;
    let connection =
        Connection::open_with_flags_and_vfs(database, flags, vfs).map_err(durability_error)?;
    Ok((connection, guard))
}

#[cfg(not(unix))]
fn open_protected_ceremony_connection(
    database: &Path,
    flags: OpenFlags,
    _existing_journal_identity: Option<ManagedRootIdentityV1>,
) -> Result<(Connection, ProtectedJournalOpenExpectationGuardV1), ProtectedCeremonyEffectErrorV1> {
    let connection = Connection::open_with_flags(database, flags).map_err(durability_error)?;
    Ok((connection, ProtectedJournalOpenExpectationGuardV1))
}

fn validate_managed_root(
    root: &Path,
) -> Result<(PathBuf, ManagedRootIdentityV1), ProtectedCeremonyEffectErrorV1> {
    validate_no_symlink_ancestors(root)?;
    let secure_root = SecureRoot::open(root).map_err(secure_durability_error)?;
    secure_root
        .verify_path_binding()
        .map_err(secure_durability_error)?;
    let canonical = secure_root.path().to_path_buf();
    let metadata = fs::metadata(&canonical).map_err(durability_error)?;
    if !metadata.is_dir() {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    let identity = metadata_identity(&metadata);
    Ok((canonical, identity))
}

fn create_ceremony_initialization_leaf(
    secure_root: &SecureRoot,
    managed_root: &Path,
) -> Result<(String, PathBuf, ManagedRootIdentityV1), ProtectedCeremonyEffectErrorV1> {
    secure_root
        .verify_path_binding()
        .map_err(secure_durability_error)?;
    for _ in 0..CEREMONY_INITIALIZATION_ATTEMPTS {
        let sequence = CEREMONY_INITIALIZATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            "protected-ceremony-init-{}-{sequence}.sqlite3",
            process::id()
        );
        if secure_root
            .create_file_if_absent(&name, &[])
            .map_err(secure_durability_error)?
            == CreateIfAbsent::AlreadyExists
        {
            continue;
        }
        let path = managed_root.join(&name);
        validate_database_leaf(managed_root, &path)?;
        let metadata = fs::symlink_metadata(&path).map_err(durability_error)?;
        if !metadata.is_file() || !metadata_has_unique_link(&metadata) {
            return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
        }
        secure_root
            .verify_path_binding()
            .map_err(secure_durability_error)?;
        return Ok((name, path, metadata_identity(&metadata)));
    }
    Err(ProtectedCeremonyEffectErrorV1::InitializationNameExhausted)
}

fn verify_managed_root(
    root: &Path,
    expected: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    validate_no_symlink_ancestors(root)?;
    let secure_root = SecureRoot::open(root).map_err(secure_durability_error)?;
    secure_root
        .verify_path_binding()
        .map_err(secure_durability_error)?;
    let metadata = fs::metadata(root).map_err(durability_error)?;
    if !metadata.is_dir() || metadata_identity(&metadata) != expected {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_identity(metadata: &fs::Metadata) -> ManagedRootIdentityV1 {
    use std::os::unix::fs::MetadataExt;

    ManagedRootIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(windows)]
fn metadata_identity(metadata: &fs::Metadata) -> ManagedRootIdentityV1 {
    use std::os::windows::fs::MetadataExt;

    ManagedRootIdentityV1 {
        device: u64::from(metadata.volume_serial_number().unwrap_or(0)),
        inode: metadata.file_index().unwrap_or(0),
    }
}

#[cfg(not(any(unix, windows)))]
fn metadata_identity(_metadata: &fs::Metadata) -> ManagedRootIdentityV1 {
    ManagedRootIdentityV1 {
        device: 0,
        inode: 0,
    }
}

fn realm_identity(
    spec: CeremonySpecV1,
    incarnation: HomeTokenV1,
    root: ManagedRootIdentityV1,
    leaf: ManagedRootIdentityV1,
) -> Result<HomeTokenV1, CborError> {
    token(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-ceremony-realm.v2")?,
        CborValue::Unsigned(root.device),
        CborValue::Unsigned(root.inode),
        CborValue::Unsigned(leaf.device),
        CborValue::Unsigned(leaf.inode),
        CborValue::Unsigned(u64::from(spec.tag())),
        bytes(incarnation.as_bytes()),
    ]))
}

fn validate_no_symlink_ancestors(path: &Path) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.exists()
            && fs::symlink_metadata(candidate)
                .map_err(durability_error)?
                .file_type()
                .is_symlink()
        {
            return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
        }
        current = candidate.parent();
    }
    Ok(())
}

fn validate_database_leaf(
    root: &Path,
    database: &Path,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    if database.parent() != Some(root)
        || database
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.starts_with("protected-ceremony-"))
        || fs::symlink_metadata(database).is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

fn verify_database_leaf(
    root: &Path,
    database: &Path,
    expected: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    validate_database_leaf(root, database)?;
    let secure_root = SecureRoot::open(root).map_err(secure_durability_error)?;
    let name = database
        .file_name()
        .ok_or(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)?;
    secure_root
        .validate_regular_file(name)
        .map_err(secure_durability_error)?;
    let metadata = fs::symlink_metadata(database).map_err(durability_error)?;
    if !metadata.is_file()
        || !metadata_has_unique_link(&metadata)
        || metadata_identity(&metadata) != expected
    {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

fn verify_rollback_journal_custody(
    root: &Path,
    database: &Path,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    rollback_journal_identity(root, database).map(|_| ())
}

fn rollback_journal_identity(
    root: &Path,
    database: &Path,
) -> Result<Option<ManagedRootIdentityV1>, ProtectedCeremonyEffectErrorV1> {
    validate_database_leaf(root, database)?;
    let database_name = database
        .file_name()
        .ok_or(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)?;
    let mut journal_name = database_name.to_os_string();
    journal_name.push("-journal");
    let secure_root = SecureRoot::open(root).map_err(secure_durability_error)?;
    let journal_path = root.join(journal_name);
    for _ in 0..ROLLBACK_JOURNAL_CUSTODY_ATTEMPTS {
        let journal_leaf = journal_path
            .file_name()
            .ok_or(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)?;
        let binding = match secure_root.bind_optional_regular_file(journal_leaf) {
            Ok(binding) => binding,
            Err(error) if rollback_journal_alias_error_is_transient(&error, &journal_path) => {
                continue;
            }
            Err(error) => return Err(secure_durability_error(error)),
        };
        secure_root
            .verify_path_binding()
            .map_err(secure_durability_error)?;
        let Some(binding) = binding else {
            return Ok(None);
        };
        let metadata = match fs::symlink_metadata(&journal_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(durability_error(error)),
        };
        match secure_root.verify_regular_file_binding(
            journal_path
                .file_name()
                .ok_or(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)?,
            &binding,
        ) {
            Ok(()) => return Ok(Some(metadata_identity(&metadata))),
            Err(SecureFsError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                continue;
            }
            Err(error) if rollback_journal_alias_error_is_transient(&error, &journal_path) => {
                continue;
            }
            Err(error) => return Err(secure_durability_error(error)),
        }
    }
    Err(ProtectedCeremonyEffectErrorV1::Durability(
        "rollback journal changed repeatedly while binding custody".to_owned(),
    ))
}

fn rollback_journal_alias_error_is_transient(error: &SecureFsError, journal: &Path) -> bool {
    if !matches!(
        error,
        SecureFsError::UnsafeObject {
            reason: "immutable file has a hard-link alias",
            ..
        }
    ) {
        return false;
    }
    match fs::symlink_metadata(journal) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => true,
        Ok(metadata) => metadata.is_file() && metadata_has_unique_link(&metadata),
        _ => false,
    }
}

#[cfg(unix)]
fn verify_open_rollback_journal(
    connection: &Connection,
    root: &Path,
    database: &Path,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    verify_rollback_journal_custody(root, database)?;
    let mut journal_file: *mut rusqlite::ffi::sqlite3_file = std::ptr::null_mut();
    let journal_pointer_result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_JOURNAL_POINTER,
            std::ptr::from_mut(&mut journal_file).cast(),
        )
    };
    if journal_pointer_result != rusqlite::ffi::SQLITE_OK
        || journal_file.is_null()
        || unsafe { (*journal_file).pMethods.is_null() }
    {
        return Err(ProtectedCeremonyEffectErrorV1::Durability(format!(
            "rollback journal descriptor is unavailable: sqlite code {journal_pointer_result}"
        )));
    }
    let Some(file_control) = (unsafe { (*(*journal_file).pMethods).xFileControl }) else {
        return Err(ProtectedCeremonyEffectErrorV1::Durability(
            "rollback journal descriptor has no file-control method".to_owned(),
        ));
    };
    let mut moved = 1_i32;
    let moved_result = unsafe {
        file_control(
            journal_file,
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            std::ptr::from_mut(&mut moved).cast(),
        )
    };
    if moved_result != rusqlite::ffi::SQLITE_OK || moved != 0 {
        return Err(ProtectedCeremonyEffectErrorV1::Durability(format!(
            "rollback journal descriptor moved or cannot be verified: sqlite code {moved_result}, moved {moved}"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_open_rollback_journal(
    _connection: &Connection,
    root: &Path,
    database: &Path,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    verify_rollback_journal_custody(root, database)
}

#[cfg(unix)]
fn verify_connection_leaf(
    connection: &Connection,
    database_path: &Path,
    expected: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    let mut sqlite_file: *mut rusqlite::ffi::sqlite3_file = std::ptr::null_mut();
    let file_pointer_result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_FILE_POINTER,
            std::ptr::from_mut(&mut sqlite_file).cast(),
        )
    };
    let mut moved = 1_i32;
    let moved_result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            std::ptr::from_mut(&mut moved).cast(),
        )
    };
    if file_pointer_result != rusqlite::ffi::SQLITE_OK
        || sqlite_file.is_null()
        || moved_result != rusqlite::ffi::SQLITE_OK
        || moved != 0
    {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    let filename =
        unsafe { rusqlite::ffi::sqlite3_db_filename(connection.handle(), c"main".as_ptr()) };
    if filename.is_null() {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    let connected_path = Path::new(
        unsafe { std::ffi::CStr::from_ptr(filename) }
            .to_str()
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
    );
    let connected_path = fs::canonicalize(connected_path).map_err(durability_error)?;
    let expected_path = fs::canonicalize(database_path).map_err(durability_error)?;
    let metadata = fs::metadata(&connected_path).map_err(durability_error)?;
    if !metadata.is_file()
        || !metadata_has_unique_link(&metadata)
        || connected_path != expected_path
        || metadata_identity(&metadata) != expected
    {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

#[cfg(windows)]
fn verify_connection_leaf(
    connection: &Connection,
    database_path: &Path,
    expected: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    use std::ffi::c_void;
    use std::mem::ManuallyDrop;
    use std::os::windows::io::FromRawHandle;

    let mut handle: *mut c_void = std::ptr::null_mut();
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_WIN32_GET_HANDLE,
            std::ptr::from_mut(&mut handle).cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK || handle.is_null() {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    let file = ManuallyDrop::new(unsafe { fs::File::from_raw_handle(handle) });
    let metadata = file.metadata().map_err(durability_error)?;
    let filename =
        unsafe { rusqlite::ffi::sqlite3_db_filename(connection.handle(), c"main".as_ptr()) };
    if filename.is_null() {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    let connected_path = Path::new(
        unsafe { std::ffi::CStr::from_ptr(filename) }
            .to_str()
            .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?,
    );
    let connected_path = fs::canonicalize(connected_path).map_err(durability_error)?;
    let expected_path = fs::canonicalize(database_path).map_err(durability_error)?;
    if !metadata.is_file()
        || !metadata_has_unique_link(&metadata)
        || connected_path != expected_path
        || metadata_identity(&metadata) != expected
    {
        return Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath);
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn verify_connection_leaf(
    _connection: &Connection,
    _database_path: &Path,
    _expected: ManagedRootIdentityV1,
) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
}

#[cfg(unix)]
fn metadata_has_unique_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.nlink() == 1
}

#[cfg(windows)]
fn metadata_has_unique_link(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.number_of_links() == Some(1)
}

#[cfg(not(any(unix, windows)))]
const fn metadata_has_unique_link(_metadata: &fs::Metadata) -> bool {
    false
}

fn require_nonzero(value: HomeTokenV1) -> Result<(), ProtectedCeremonyEffectErrorV1> {
    if value.as_bytes() == &[0; 32] {
        Err(ProtectedCeremonyEffectErrorV1::MissingCommitment)
    } else {
        Ok(())
    }
}

fn token(value: &CborValue) -> Result<HomeTokenV1, CborError> {
    Ok(HomeTokenV1::new(
        Sha256::digest(deterministic_cbor::encode(value)?).into(),
    ))
}

fn token_from_bytes(value: &[u8]) -> Result<HomeTokenV1, ProtectedCeremonyEffectErrorV1> {
    let bytes: [u8; 32] = value
        .try_into()
        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
    if bytes == [0; 32] {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    }
    Ok(HomeTokenV1::new(bytes))
}

fn token_from_cbor(value: &CborValue) -> Result<HomeTokenV1, ProtectedCeremonyEffectErrorV1> {
    let CborValue::Bytes(value) = value else {
        return Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier);
    };
    token_from_bytes(value)
}

fn exact_u64(value: &[u8]) -> Result<u64, ProtectedCeremonyEffectErrorV1> {
    let bytes: [u8; 8] = value
        .try_into()
        .map_err(|_| ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)?;
    Ok(u64::from_be_bytes(bytes))
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn durability_error(error: impl std::fmt::Display) -> ProtectedCeremonyEffectErrorV1 {
    ProtectedCeremonyEffectErrorV1::Durability(error.to_string())
}

fn secure_durability_error(error: impl std::fmt::Display) -> ProtectedCeremonyEffectErrorV1 {
    ProtectedCeremonyEffectErrorV1::Durability(error.to_string())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProtectedCeremonyEffectErrorV1 {
    #[error("protected Ceremony effect commitment must not be all zero")]
    MissingCommitment,
    #[error("protected Ceremony effect expected-old carrier is stale or cross-home")]
    StaleExpectedCarrier,
    #[error("protected Ceremony effect carrier revision overflowed")]
    RevisionOverflow,
    #[error("protected Ceremony initialization could not allocate a unique temporary carrier")]
    InitializationNameExhausted,
    #[error("protected Ceremony effect carrier identity does not match the requested Ceremony")]
    CarrierIdentityMismatch,
    #[error("protected Ceremony effect request has no exact current owner-issued authority")]
    InvalidAuthorityCarrier,
    #[error("protected Ceremony effect mode is illegal for the current phase")]
    IllegalLifecycleTransition,
    #[error("protected Ceremony idempotency key was reused with different meaning")]
    IdempotencyMeaningConflict,
    #[error("protected Ceremony effect carrier persistence is malformed")]
    InvalidPersistentCarrier,
    #[error("protected Ceremony effect carrier path is unsafe")]
    UnsafePersistentCarrierPath,
    #[error("protected Ceremony effect durability failed: {0}")]
    Durability(String),
    #[error(transparent)]
    Route(#[from] super::effect_routes::RouteCompatibilityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    const MODES: [CeremonyRequestModeV1; 4] = [
        CeremonyRequestModeV1::Initiate,
        CeremonyRequestModeV1::RecoverReserved,
        CeremonyRequestModeV1::ResolveResult,
        CeremonyRequestModeV1::Withdraw,
    ];

    #[test]
    fn all_eleven_ceremonies_have_exactly_four_owner_issued_legal_durable_modes() {
        let mut admitted = 0;
        let mut withdrawal_cells = 0;
        for spec in CeremonySpecV1::ALL {
            for mode in MODES {
                let root = test_root(spec.name());
                let incarnation = HomeTokenV1::new([spec.tag(); 32]);
                let basis = HomeTokenV1::new([40 + spec.tag(); 32]);
                let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
                let (store, anchor) =
                    ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &owner)
                        .unwrap();
                prepare_phase(&owner, &store, mode);
                let carrier = store.current().unwrap();
                let request = owner
                    .issue_request(
                        &store,
                        mode,
                        carrier.current_token(),
                        HomeTokenV1::new([80 + mode_tag(mode) as u8; 32]),
                        HomeTokenV1::new([120 + mode_tag(mode) as u8; 32]),
                    )
                    .unwrap();
                assert_eq!(request.mode(), mode);
                assert!(!request.creates_attempt_or_run());
                if mode == CeremonyRequestModeV1::Withdraw {
                    for classification in [
                        RemoteClassificationV1::Prepared,
                        RemoteClassificationV1::ConfirmedNotApplied,
                    ] {
                        let cell = request.withdrawal_catalog_cell(classification).unwrap();
                        assert_eq!(cell.classification(), classification);
                        assert_eq!(cell.home(), spec.home_kind());
                        assert_eq!(cell.branch_tag(), u64::from(spec.tag()));
                        assert_eq!(cell.role_literal(), "CeremonyWithdraw");
                        withdrawal_cells += 1;
                    }
                }
                let outcome = store.publish(request).unwrap();
                assert_eq!(outcome.provider_io_operations(), 0);
                assert!(!outcome.replayed());
                let reopened =
                    ProtectedCeremonyEffectStoreV1::open(&root, &anchor, &owner).unwrap();
                assert_eq!(
                    reopened.current().unwrap().current_token(),
                    outcome.current_token()
                );
                admitted += 1;
                let _ = fs::remove_dir_all(root);
            }
        }
        assert_eq!(admitted, 44);
        assert_eq!(withdrawal_cells, 22);
    }

    #[test]
    fn protected_ceremony_reopens_from_durable_anchor_and_rejects_inode_replacement() {
        let spec = CeremonySpecV1::RecoverRepositoryStoreGeneration;
        let incarnation = HomeTokenV1::new([61; 32]);
        let basis = HomeTokenV1::new([62; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let root = test_root("ceremony-durable-anchor");
        let (store, anchor) =
            ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &owner).unwrap();
        let anchor_bytes = anchor.canonical_bytes().unwrap();
        let carrier = store.current().unwrap();
        let request = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                carrier.current_token(),
                HomeTokenV1::new([63; 32]),
                HomeTokenV1::new([64; 32]),
            )
            .unwrap();
        let committed = store.publish(request).unwrap();
        let database_path = store.database_path.clone();
        drop(store);

        let durable_anchor =
            ProtectedCeremonyCarrierAnchorV1::from_canonical_bytes(&anchor_bytes).unwrap();
        let reopened =
            ProtectedCeremonyEffectStoreV1::open(&root, &durable_anchor, &owner).unwrap();
        assert_eq!(
            reopened.current().unwrap().current_token(),
            committed.current_token()
        );

        let displaced = root.join("displaced.sqlite3");
        fs::rename(&database_path, &displaced).unwrap();
        fs::copy(&displaced, &database_path).unwrap();
        let displaced_connection = Connection::open(&displaced).unwrap();
        assert!(matches!(
            reopened.verify_live_connection(&displaced_connection),
            Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
        ));
        drop(displaced_connection);
        let replacement_identity =
            metadata_identity(&fs::symlink_metadata(&database_path).unwrap());
        let replacement = Connection::open(&database_path).unwrap();
        assert!(matches!(
            verify_connection_leaf(&replacement, &database_path, anchor.leaf_identity),
            Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
        ));
        replacement
            .execute(
                "UPDATE protected_ceremony_carrier_anchor
                 SET leaf_device = ?1, leaf_inode = ?2 WHERE singleton = 1",
                params![
                    replacement_identity.device.to_be_bytes().as_slice(),
                    replacement_identity.inode.to_be_bytes().as_slice(),
                ],
            )
            .unwrap();
        drop(replacement);
        assert!(matches!(
            ProtectedCeremonyEffectStoreV1::open(&root, &durable_anchor, &owner),
            Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch)
        ));
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn protected_ceremony_rejects_unsafe_owner_writable_modes_and_journal_aliases() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = test_root("ceremony-custody");
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([91; 32]))
            .unwrap();
        let (store, _) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RepositoryV1Cutover,
            HomeTokenV1::new([92; 32]),
            &owner,
        )
        .unwrap();
        let database = store.database_path.clone();

        fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(
            store.current(),
            Err(ProtectedCeremonyEffectErrorV1::Durability(_))
                | Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
        ));
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();

        fs::set_permissions(&database, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(matches!(
            store.current(),
            Err(ProtectedCeremonyEffectErrorV1::Durability(_))
                | Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
        ));
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();

        let journal = PathBuf::from(format!("{}-journal", database.display()));
        let foreign = root.join("foreign-journal-target");
        fs::write(&foreign, b"foreign").unwrap();
        symlink(&foreign, &journal).unwrap();
        assert!(store.current().is_err());
        fs::remove_file(&journal).unwrap();

        fs::hard_link(&foreign, &journal).unwrap();
        assert!(store.current().is_err());
        fs::remove_file(&journal).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn protected_ceremony_journal_creation_rejects_hardlink_substitution_at_open() {
        let root = test_root("ceremony-journal-open-race");
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([93; 32]))
            .unwrap();
        let (store, _) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RepositoryV1Cutover,
            HomeTokenV1::new([94; 32]),
            &owner,
        )
        .unwrap();
        let current = store.current().unwrap();
        let request = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                current.current_token(),
                HomeTokenV1::new([95; 32]),
                HomeTokenV1::new([96; 32]),
            )
            .unwrap();
        let journal = rollback_journal_path(&store.database_path);
        let foreign = root.join("foreign-journal-open-target");
        fs::write(&foreign, b"foreign-journal-bytes").unwrap();
        let reached_open = Arc::new(Barrier::new(2));
        let continue_open = Arc::new(Barrier::new(2));
        *PROTECTED_JOURNAL_OPEN_TEST_HOOK.lock().unwrap() = Some(ProtectedJournalOpenTestHookV1 {
            path: journal.clone(),
            reached_open: Arc::clone(&reached_open),
            continue_open: Arc::clone(&continue_open),
        });
        let worker = std::thread::spawn(move || store.publish(request));
        reached_open.wait();
        assert!(!journal.exists());
        fs::hard_link(&foreign, &journal).unwrap();
        continue_open.wait();
        let outcome = worker.join().unwrap();
        *PROTECTED_JOURNAL_OPEN_TEST_HOOK.lock().unwrap() = None;

        assert!(outcome.is_err());
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign-journal-bytes");
        fs::remove_file(&journal).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protected_ceremony_rejects_corrupted_transition_history() {
        let root = test_root("ceremony-history-corruption");
        let basis = HomeTokenV1::new([66; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let (store, anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RepositoryV1Cutover,
            HomeTokenV1::new([65; 32]),
            &owner,
        )
        .unwrap();
        let current = store.current().unwrap();
        let request = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                current.current_token(),
                HomeTokenV1::new([67; 32]),
                HomeTokenV1::new([68; 32]),
            )
            .unwrap();
        store.publish(request).unwrap();
        let database_path = store.database_path.clone();
        drop(store);
        let connection = Connection::open(database_path).unwrap();
        connection
            .execute(
                "UPDATE protected_ceremony_effect_idempotency
                 SET authority_commitment = ?1",
                params![[69_u8; 32].as_slice()],
            )
            .unwrap();
        drop(connection);
        assert!(matches!(
            ProtectedCeremonyEffectStoreV1::open(&root, &anchor, &owner),
            Err(ProtectedCeremonyEffectErrorV1::InvalidPersistentCarrier)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn abandoned_initialization_leaf_does_not_publish_or_poison_final_incarnation() {
        let root = test_root("ceremony-abandoned-initialization");
        fs::write(
            root.join("protected-ceremony-init-abandoned.sqlite3"),
            b"incomplete",
        )
        .unwrap();
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([71; 32]))
            .unwrap();
        let (store, anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RecoverInstallationStoreGeneration,
            HomeTokenV1::new([70; 32]),
            &owner,
        )
        .unwrap();
        assert_eq!(store.current().unwrap().revision(), 1);
        assert!(ProtectedCeremonyEffectStoreV1::open(&root, &anchor, &owner).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protected_ceremony_debug_redacts_every_bearer_token() {
        let root = test_root("ceremony-debug-redaction");
        let basis = HomeTokenV1::new([73; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let candidate = HomeTokenV1::new([74; 32]);
        let idempotency_key = HomeTokenV1::new([75; 32]);
        let (store, anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RecoverPreStoreWriterCohort,
            HomeTokenV1::new([72; 32]),
            &owner,
        )
        .unwrap();
        let carrier = store.current().unwrap();
        let request = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                carrier.current_token(),
                candidate,
                idempotency_key,
            )
            .unwrap();
        let rendered = format!("{owner:?} {store:?} {anchor:?} {carrier:?} {request:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(
            !anchor
                .canonical_bytes()
                .unwrap()
                .windows(basis.as_bytes().len())
                .any(|window| window == basis.as_bytes())
        );
        for secret in [
            basis,
            carrier.current_token(),
            candidate,
            idempotency_key,
            request.id(),
        ] {
            assert!(!rendered.contains(&hex(secret.as_bytes())));
            assert!(!rendered.contains(&format!("{:?}", secret.as_bytes())));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ceremony_readers_share_one_snapshot_with_concurrent_writer_history_validation() {
        let root = test_root("ceremony-reader-writer-snapshot");
        let basis = HomeTokenV1::new([76; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let (store, _anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::InstallationV1Cutover,
            HomeTokenV1::new([77; 32]),
            &owner,
        )
        .unwrap();
        let store = Arc::new(store);
        let started = Arc::new(Barrier::new(2));
        let finished = Arc::new(AtomicBool::new(false));
        let reader = {
            let store = Arc::clone(&store);
            let started = Arc::clone(&started);
            let finished = Arc::clone(&finished);
            std::thread::spawn(move || {
                started.wait();
                let mut observations = 0;
                while !finished.load(Ordering::Acquire) {
                    store.current().unwrap();
                    observations += 1;
                }
                store.current().unwrap();
                observations
            })
        };
        started.wait();
        for (mode, candidate, idempotency) in [
            (CeremonyRequestModeV1::Initiate, 78, 79),
            (CeremonyRequestModeV1::RecoverReserved, 80, 81),
            (CeremonyRequestModeV1::ResolveResult, 82, 83),
        ] {
            let current = store.current().unwrap();
            let request = owner
                .issue_request(
                    &store,
                    mode,
                    current.current_token(),
                    HomeTokenV1::new([candidate; 32]),
                    HomeTokenV1::new([idempotency; 32]),
                )
                .unwrap();
            store.publish(request).unwrap();
        }
        finished.store(true, Ordering::Release);
        assert!(reader.join().unwrap() > 0);
        assert_eq!(store.current().unwrap().revision(), 4);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protected_ceremony_has_one_winner_durable_full_history_replay_and_owner_refusal() {
        let spec = CeremonySpecV1::InstallationContextGenesis;
        let incarnation = HomeTokenV1::new([1; 32]);
        let basis = HomeTokenV1::new([2; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let root = test_root("ceremony-race");
        let (store, anchor) =
            ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &owner).unwrap();
        let carrier = store.current().unwrap();
        let request_a = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                carrier.current_token(),
                HomeTokenV1::new([3; 32]),
                HomeTokenV1::new([4; 32]),
            )
            .unwrap();
        let durable_request = request_a.canonical_bytes().unwrap();
        let duplicate_a = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                carrier.current_token(),
                HomeTokenV1::new([3; 32]),
                HomeTokenV1::new([4; 32]),
            )
            .unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let workers =
            [(store.clone(), request_a), (store.clone(), duplicate_a)].map(|(store, request)| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store.publish(request)
                })
            });
        barrier.wait();
        let outcomes = workers.map(|worker| worker.join().unwrap().unwrap());
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.replayed()).count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| !outcome.replayed())
                .count(),
            1
        );
        assert_eq!(outcomes[0].current_token(), outcomes[1].current_token());

        let reserved = store.current().unwrap();
        let replay_after_intervening = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::RecoverReserved,
                reserved.current_token(),
                HomeTokenV1::new([5; 32]),
                HomeTokenV1::new([6; 32]),
            )
            .unwrap();
        store.publish(replay_after_intervening).unwrap();
        let reopened = ProtectedCeremonyEffectStoreV1::open(&root, &anchor, &owner).unwrap();
        let recovered_request = owner.decode_request(&reopened, &durable_request).unwrap();
        let historical_duplicate = reopened.publish(recovered_request);
        assert!(historical_duplicate.unwrap().replayed());

        let other_root = test_root("ceremony-other-owner");
        let other_owner =
            ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([7; 32])).unwrap();
        let (other, _other_anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &other_root,
            spec,
            incarnation,
            &other_owner,
        )
        .unwrap();
        let other_carrier = other.current().unwrap();
        let wrong_owner_request = other_owner
            .issue_request(
                &other,
                CeremonyRequestModeV1::Withdraw,
                other_carrier.current_token(),
                HomeTokenV1::new([8; 32]),
                HomeTokenV1::new([9; 32]),
            )
            .unwrap();
        assert!(matches!(
            other_owner.decode_request(&reopened, &durable_request),
            Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier)
        ));
        assert!(matches!(
            store.publish(wrong_owner_request),
            Err(ProtectedCeremonyEffectErrorV1::InvalidAuthorityCarrier)
                | Err(ProtectedCeremonyEffectErrorV1::IdempotencyMeaningConflict)
        ));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(other_root);
    }

    #[test]
    fn ceremony_refuses_virgin_resolve_post_terminal_mutation_and_symlink_root() {
        let root = test_root("ceremony-illegal");
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([11; 32]))
            .unwrap();
        let (store, anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            &root,
            CeremonySpecV1::RepositoryV1Cutover,
            HomeTokenV1::new([10; 32]),
            &owner,
        )
        .unwrap();
        let carrier = store.current().unwrap();
        assert!(matches!(
            owner.issue_request(
                &store,
                CeremonyRequestModeV1::ResolveResult,
                carrier.current_token(),
                HomeTokenV1::new([12; 32]),
                HomeTokenV1::new([13; 32]),
            ),
            Err(ProtectedCeremonyEffectErrorV1::IllegalLifecycleTransition)
        ));
        let withdraw = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Withdraw,
                carrier.current_token(),
                HomeTokenV1::new([14; 32]),
                HomeTokenV1::new([15; 32]),
            )
            .unwrap();
        store.publish(withdraw).unwrap();
        let terminal = store.current().unwrap();
        assert!(matches!(
            owner.issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                terminal.current_token(),
                HomeTokenV1::new([16; 32]),
                HomeTokenV1::new([17; 32]),
            ),
            Err(ProtectedCeremonyEffectErrorV1::IllegalLifecycleTransition)
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let alias_parent = test_root("ceremony-symlink-parent");
            let alias = alias_parent.join("alias");
            symlink(&root, &alias).unwrap();
            assert!(matches!(
                ProtectedCeremonyEffectStoreV1::open(alias, &anchor, &owner),
                Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
            ));
            let _ = fs::remove_dir_all(alias_parent);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ceremony_anchor_rejects_leaf_replacement_and_recovers_published_incarnation() {
        let root = test_root("ceremony-leaf-anchor");
        let spec = CeremonySpecV1::RecoverPreStoreBinarySlot;
        let incarnation = HomeTokenV1::new([31; 32]);
        let basis = HomeTokenV1::new([32; 32]);
        let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(basis).unwrap();
        let (store, anchor) =
            ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &owner).unwrap();
        #[cfg(unix)]
        {
            let alias = root.join("carrier-hard-link.sqlite3");
            fs::hard_link(&store.database_path, &alias).unwrap();
            assert!(matches!(
                store.current(),
                Err(ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath)
                    | Err(ProtectedCeremonyEffectErrorV1::Durability(_))
            ));
            fs::remove_file(alias).unwrap();
        }
        let (recovered, recovered_anchor) =
            ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &owner).unwrap();
        assert_eq!(recovered_anchor, anchor);
        assert_eq!(recovered.current().unwrap(), store.current().unwrap());
        let wrong_owner =
            ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([35; 32]))
                .unwrap();
        assert!(matches!(
            ProtectedCeremonyEffectStoreV1::initialize(&root, spec, incarnation, &wrong_owner),
            Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch)
        ));
        let stale_copy = root.join("stale-carrier-copy.sqlite3");
        fs::copy(&store.database_path, &stale_copy).unwrap();
        let carrier = store.current().unwrap();
        let request = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                carrier.current_token(),
                HomeTokenV1::new([33; 32]),
                HomeTokenV1::new([34; 32]),
            )
            .unwrap();
        store.publish(request).unwrap();
        let database_path = store.database_path.clone();
        drop(store);
        fs::remove_file(&database_path).unwrap();
        fs::rename(stale_copy, &database_path).unwrap();
        assert!(matches!(
            ProtectedCeremonyEffectStoreV1::open(&root, &anchor, &owner),
            Err(ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch)
        ));
        let _ = fs::remove_dir_all(root);
    }

    fn prepare_phase(
        owner: &ProtectedCeremonyOwnerAuthorityV1,
        store: &ProtectedCeremonyEffectStoreV1,
        target: CeremonyRequestModeV1,
    ) {
        if matches!(
            target,
            CeremonyRequestModeV1::Initiate | CeremonyRequestModeV1::Withdraw
        ) {
            return;
        }
        let empty = store.current().unwrap();
        let initiate = owner
            .issue_request(
                store,
                CeremonyRequestModeV1::Initiate,
                empty.current_token(),
                HomeTokenV1::new([20; 32]),
                HomeTokenV1::new([21; 32]),
            )
            .unwrap();
        store.publish(initiate).unwrap();
        if matches!(target, CeremonyRequestModeV1::ResolveResult) {
            let reserved = store.current().unwrap();
            let recover = owner
                .issue_request(
                    store,
                    CeremonyRequestModeV1::RecoverReserved,
                    reserved.current_token(),
                    HomeTokenV1::new([22; 32]),
                    HomeTokenV1::new([23; 32]),
                )
                .unwrap();
            store.publish(recover).unwrap();
        }
    }

    fn test_root(seed: &str) -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!(
                "maestro-vnext-stage4-ceremony-{}-{counter}-{nanos}",
                std::process::id()
            ))
            .join(seed);
        fs::create_dir_all(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }
}
