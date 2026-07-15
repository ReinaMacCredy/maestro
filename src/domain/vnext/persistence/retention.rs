use thiserror::Error;

use crate::domain::vnext::identity::{
    CollectionPlanIdV1, IdentityKindV1, LogicalTombstoneIdV1, ManifestIdentityV1,
    ReachabilitySnapshotIdV1, RetentionPinIdV1, StoreHeadIdV1, StoreObjectIdV1, derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const MAX_RETENTION_ROOTS: usize = 65_536;
pub const MAX_REACHABLE_OBJECTS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionRootKindV1 {
    ActiveGeneration,
    CandidateSeal,
    UnresolvedEffect,
    RecoveryBundle,
    RollbackBundle,
    LegalHold,
    ReplayHorizon,
    ProtocolClosure,
    BinaryClosure,
    ResourceClosure,
    SealedExport,
    MigrationAssociation,
    Quarantine,
    RecoveryCommitment,
}

impl RetentionRootKindV1 {
    pub const ALL: [Self; 14] = [
        Self::ActiveGeneration,
        Self::CandidateSeal,
        Self::UnresolvedEffect,
        Self::RecoveryBundle,
        Self::RollbackBundle,
        Self::LegalHold,
        Self::ReplayHorizon,
        Self::ProtocolClosure,
        Self::BinaryClosure,
        Self::ResourceClosure,
        Self::SealedExport,
        Self::MigrationAssociation,
        Self::Quarantine,
        Self::RecoveryCommitment,
    ];

    pub const fn tag(self) -> u64 {
        match self {
            Self::ActiveGeneration => 1,
            Self::CandidateSeal => 2,
            Self::UnresolvedEffect => 3,
            Self::RecoveryBundle => 4,
            Self::RollbackBundle => 5,
            Self::LegalHold => 6,
            Self::ReplayHorizon => 7,
            Self::ProtocolClosure => 8,
            Self::BinaryClosure => 9,
            Self::ResourceClosure => 10,
            Self::SealedExport => 11,
            Self::MigrationAssociation => 12,
            Self::Quarantine => 13,
            Self::RecoveryCommitment => 14,
        }
    }

    pub(crate) fn from_tag(tag: u64) -> Result<Self, RetentionError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.tag() == tag)
            .ok_or(RetentionError::UnknownRootKind(tag))
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetentionRootV1 {
    kind: RetentionRootKindV1,
    object_id: StoreObjectIdV1,
}

impl RetentionRootV1 {
    pub fn new(kind: RetentionRootKindV1, object_id: StoreObjectIdV1) -> Self {
        Self { kind, object_id }
    }

    pub fn kind(&self) -> RetentionRootKindV1 {
        self.kind
    }

    pub fn object_id(&self) -> StoreObjectIdV1 {
        self.object_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            CborValue::Bytes(self.object_id.as_bytes().to_vec()),
        ])
    }

    fn decode(value: &CborValue) -> Result<Self, RetentionError> {
        let CborValue::Array(fields) = value else {
            return Err(RetentionError::InvalidShape);
        };
        let [CborValue::Unsigned(kind), CborValue::Bytes(object_id)] = fields.as_slice() else {
            return Err(RetentionError::InvalidShape);
        };
        Ok(Self::new(
            RetentionRootKindV1::from_tag(*kind)?,
            identity_from_bytes(object_id)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionPinV1 {
    basis_head_id: StoreHeadIdV1,
    root: RetentionRootV1,
    reason_digest: [u8; 32],
    id: RetentionPinIdV1,
}

impl RetentionPinV1 {
    pub fn new(
        basis_head_id: StoreHeadIdV1,
        root: RetentionRootV1,
        reason_digest: [u8; 32],
    ) -> Result<Self, RetentionError> {
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(basis_head_id.as_bytes().to_vec()),
            root.canonical_value(),
            CborValue::Bytes(reason_digest.to_vec()),
        ]);
        let id = derive_identity(&value)?;
        Ok(Self {
            basis_head_id,
            root,
            reason_digest,
            id,
        })
    }

    pub fn basis_head_id(&self) -> StoreHeadIdV1 {
        self.basis_head_id
    }

    pub fn root(&self) -> &RetentionRootV1 {
        &self.root
    }

    pub fn reason_digest(&self) -> &[u8; 32] {
        &self.reason_digest
    }

    pub fn id(&self) -> RetentionPinIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(self.basis_head_id.as_bytes().to_vec()),
            self.root.canonical_value(),
            CborValue::Bytes(self.reason_digest.to_vec()),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RetentionError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(RetentionError::InvalidShape);
        };
        let [
            CborValue::Unsigned(1),
            CborValue::Bytes(head),
            root,
            CborValue::Bytes(reason),
        ] = fields.as_slice()
        else {
            return Err(RetentionError::InvalidShape);
        };
        let pin = Self::new(
            identity_from_bytes(head)?,
            RetentionRootV1::decode(root)?,
            digest_from_bytes(reason)?,
        )?;
        if pin.canonical_bytes()? != bytes {
            return Err(RetentionError::NonCanonicalBytes);
        }
        Ok(pin)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalTombstoneV1 {
    basis_head_id: StoreHeadIdV1,
    object_id: StoreObjectIdV1,
    reason_digest: [u8; 32],
    invalidation_digest: [u8; 32],
    id: LogicalTombstoneIdV1,
}

impl LogicalTombstoneV1 {
    pub fn new(
        basis_head_id: StoreHeadIdV1,
        object_id: StoreObjectIdV1,
        reason_digest: [u8; 32],
        invalidation_digest: [u8; 32],
    ) -> Result<Self, RetentionError> {
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(basis_head_id.as_bytes().to_vec()),
            CborValue::Bytes(object_id.as_bytes().to_vec()),
            CborValue::Bytes(reason_digest.to_vec()),
            CborValue::Bytes(invalidation_digest.to_vec()),
        ]);
        let id = derive_identity(&value)?;
        Ok(Self {
            basis_head_id,
            object_id,
            reason_digest,
            invalidation_digest,
            id,
        })
    }

    pub fn basis_head_id(&self) -> StoreHeadIdV1 {
        self.basis_head_id
    }

    pub fn object_id(&self) -> StoreObjectIdV1 {
        self.object_id
    }

    pub fn reason_digest(&self) -> &[u8; 32] {
        &self.reason_digest
    }

    pub fn invalidation_digest(&self) -> &[u8; 32] {
        &self.invalidation_digest
    }

    pub fn id(&self) -> LogicalTombstoneIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(self.basis_head_id.as_bytes().to_vec()),
            CborValue::Bytes(self.object_id.as_bytes().to_vec()),
            CborValue::Bytes(self.reason_digest.to_vec()),
            CborValue::Bytes(self.invalidation_digest.to_vec()),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RetentionError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(RetentionError::InvalidShape);
        };
        let [
            CborValue::Unsigned(1),
            CborValue::Bytes(head),
            CborValue::Bytes(object),
            CborValue::Bytes(reason),
            CborValue::Bytes(invalidation),
        ] = fields.as_slice()
        else {
            return Err(RetentionError::InvalidShape);
        };
        let tombstone = Self::new(
            identity_from_bytes(head)?,
            identity_from_bytes(object)?,
            digest_from_bytes(reason)?,
            digest_from_bytes(invalidation)?,
        )?;
        if tombstone.canonical_bytes()? != bytes {
            return Err(RetentionError::NonCanonicalBytes);
        }
        Ok(tombstone)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReachabilitySnapshotV1 {
    head_id: StoreHeadIdV1,
    retention_revision: u64,
    roots: Vec<RetentionRootV1>,
    reachable: Vec<StoreObjectIdV1>,
    tombstoned: Vec<StoreObjectIdV1>,
    id: ReachabilitySnapshotIdV1,
}

impl ReachabilitySnapshotV1 {
    pub fn new(
        head_id: StoreHeadIdV1,
        retention_revision: u64,
        roots: Vec<RetentionRootV1>,
        reachable: Vec<StoreObjectIdV1>,
        tombstoned: Vec<StoreObjectIdV1>,
    ) -> Result<Self, RetentionError> {
        if retention_revision == 0 {
            return Err(RetentionError::ZeroRetentionRevision);
        }
        if roots.len() > MAX_RETENTION_ROOTS
            || reachable.len() > MAX_REACHABLE_OBJECTS
            || tombstoned.len() > MAX_REACHABLE_OBJECTS
        {
            return Err(RetentionError::ClosureTooLarge);
        }
        if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RetentionError::RootsNotStrictlySorted);
        }
        if !strictly_sorted(&reachable) || !strictly_sorted(&tombstoned) {
            return Err(RetentionError::ObjectsNotStrictlySorted);
        }
        if roots
            .iter()
            .any(|root| reachable.binary_search(&root.object_id()).is_err())
        {
            return Err(RetentionError::RootNotReachable);
        }
        if reachable
            .iter()
            .any(|object| tombstoned.binary_search(object).is_ok())
        {
            return Err(RetentionError::ReachableObjectTombstoned);
        }
        let value = snapshot_value(head_id, retention_revision, &roots, &reachable, &tombstoned);
        let id = derive_identity(&value)?;
        Ok(Self {
            head_id,
            retention_revision,
            roots,
            reachable,
            tombstoned,
            id,
        })
    }

    pub fn head_id(&self) -> StoreHeadIdV1 {
        self.head_id
    }

    pub fn retention_revision(&self) -> u64 {
        self.retention_revision
    }

    pub fn roots(&self) -> &[RetentionRootV1] {
        &self.roots
    }

    pub fn reachable(&self) -> &[StoreObjectIdV1] {
        &self.reachable
    }

    pub fn tombstoned(&self) -> &[StoreObjectIdV1] {
        &self.tombstoned
    }

    pub fn id(&self) -> ReachabilitySnapshotIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&snapshot_value(
            self.head_id,
            self.retention_revision,
            &self.roots,
            &self.reachable,
            &self.tombstoned,
        ))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RetentionError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(RetentionError::InvalidShape);
        };
        let [
            CborValue::Unsigned(1),
            CborValue::Bytes(head),
            CborValue::Unsigned(revision),
            CborValue::Array(roots),
            CborValue::Array(reachable),
            CborValue::Array(tombstoned),
        ] = fields.as_slice()
        else {
            return Err(RetentionError::InvalidShape);
        };
        let roots = roots
            .iter()
            .map(RetentionRootV1::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = Self::new(
            identity_from_bytes(head)?,
            *revision,
            roots,
            decode_identity_array(reachable)?,
            decode_identity_array(tombstoned)?,
        )?;
        if snapshot.canonical_bytes()? != bytes {
            return Err(RetentionError::NonCanonicalBytes);
        }
        Ok(snapshot)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionPlanV1 {
    snapshot_id: ReachabilitySnapshotIdV1,
    head_id: StoreHeadIdV1,
    retention_revision: u64,
    candidates: Vec<StoreObjectIdV1>,
    id: CollectionPlanIdV1,
}

impl CollectionPlanV1 {
    pub fn new(
        snapshot: &ReachabilitySnapshotV1,
        candidates: Vec<StoreObjectIdV1>,
    ) -> Result<Self, RetentionError> {
        if !strictly_sorted(&candidates) {
            return Err(RetentionError::ObjectsNotStrictlySorted);
        }
        if candidates.iter().any(|candidate| {
            snapshot.reachable.binary_search(candidate).is_ok()
                || snapshot.tombstoned.binary_search(candidate).is_err()
        }) {
            return Err(RetentionError::CandidateNotCollectable);
        }
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(snapshot.id().as_bytes().to_vec()),
            CborValue::Bytes(snapshot.head_id().as_bytes().to_vec()),
            CborValue::Unsigned(snapshot.retention_revision()),
            identity_array(&candidates),
        ]);
        let id = derive_identity(&value)?;
        Ok(Self {
            snapshot_id: snapshot.id(),
            head_id: snapshot.head_id(),
            retention_revision: snapshot.retention_revision(),
            candidates,
            id,
        })
    }

    pub fn snapshot_id(&self) -> ReachabilitySnapshotIdV1 {
        self.snapshot_id
    }

    pub fn head_id(&self) -> StoreHeadIdV1 {
        self.head_id
    }

    pub fn retention_revision(&self) -> u64 {
        self.retention_revision
    }

    pub fn candidates(&self) -> &[StoreObjectIdV1] {
        &self.candidates
    }

    pub fn id(&self) -> CollectionPlanIdV1 {
        self.id
    }
}

fn snapshot_value(
    head_id: StoreHeadIdV1,
    retention_revision: u64,
    roots: &[RetentionRootV1],
    reachable: &[StoreObjectIdV1],
    tombstoned: &[StoreObjectIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(head_id.as_bytes().to_vec()),
        CborValue::Unsigned(retention_revision),
        CborValue::Array(roots.iter().map(RetentionRootV1::canonical_value).collect()),
        identity_array(reachable),
        identity_array(tombstoned),
    ])
}

fn identity_array(ids: &[StoreObjectIdV1]) -> CborValue {
    CborValue::Array(
        ids.iter()
            .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
            .collect(),
    )
}

fn strictly_sorted(ids: &[StoreObjectIdV1]) -> bool {
    ids.windows(2).all(|pair| pair[0] < pair[1])
}

fn decode_identity_array(values: &[CborValue]) -> Result<Vec<StoreObjectIdV1>, RetentionError> {
    values
        .iter()
        .map(|value| {
            let CborValue::Bytes(bytes) = value else {
                return Err(RetentionError::InvalidShape);
            };
            identity_from_bytes(bytes)
        })
        .collect()
}

fn identity_from_bytes<K>(bytes: &[u8]) -> Result<ManifestIdentityV1<K>, RetentionError>
where
    K: IdentityKindV1,
{
    Ok(ManifestIdentityV1::from_digest(digest_from_bytes(bytes)?))
}

fn digest_from_bytes(bytes: &[u8]) -> Result<[u8; 32], RetentionError> {
    bytes
        .try_into()
        .map_err(|_| RetentionError::InvalidIdentityLength)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RetentionError {
    #[error("retention carrier has an invalid canonical shape")]
    InvalidShape,
    #[error("retention identity or digest must contain exactly 32 bytes")]
    InvalidIdentityLength,
    #[error("unknown retention root kind tag {0}")]
    UnknownRootKind(u64),
    #[error("retention carrier bytes are not the exact canonical encoding")]
    NonCanonicalBytes,
    #[error("retention revision must be positive")]
    ZeroRetentionRevision,
    #[error("retention or reachability closure exceeds the finite v1 limit")]
    ClosureTooLarge,
    #[error("retention roots must be strictly kind-and-identity sorted and unique")]
    RootsNotStrictlySorted,
    #[error("reachability object identities must be strictly sorted and unique")]
    ObjectsNotStrictlySorted,
    #[error("every retention root must belong to the verified reachable closure")]
    RootNotReachable,
    #[error("a logically tombstoned object cannot remain reachable")]
    ReachableObjectTombstoned,
    #[error("collection candidates must be tombstoned and unreachable")]
    CandidateNotCollectable,
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
