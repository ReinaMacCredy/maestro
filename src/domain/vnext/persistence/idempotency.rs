use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::domain::vnext::identity::{StoreHeadIdV1, StoreObjectIdV1};

use super::{StoreGenerationV1, StoreHeadV1, StoreObjectV1};

const MAX_IDEMPOTENCY_NAMESPACE_BYTES_V1: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreIdempotencyProbeV1 {
    namespace: String,
    key_digest: [u8; 32],
    meaning_digest: [u8; 32],
}

impl StoreIdempotencyProbeV1 {
    pub fn new(
        namespace: impl Into<String>,
        key_digest: [u8; 32],
        meaning_digest: [u8; 32],
    ) -> Result<Self, AtomicPublicationError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        Ok(Self {
            namespace,
            key_digest,
            meaning_digest,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn key_digest(&self) -> &[u8; 32] {
        &self.key_digest
    }

    pub fn meaning_digest(&self) -> &[u8; 32] {
        &self.meaning_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreIdempotencyV1 {
    namespace: String,
    key_digest: [u8; 32],
    meaning_digest: [u8; 32],
    result_object_id: StoreObjectIdV1,
}

impl StoreIdempotencyV1 {
    pub fn new(
        namespace: impl Into<String>,
        key_digest: [u8; 32],
        meaning_digest: [u8; 32],
        result_object_id: StoreObjectIdV1,
    ) -> Result<Self, AtomicPublicationError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        Ok(Self {
            namespace,
            key_digest,
            meaning_digest,
            result_object_id,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn key_digest(&self) -> &[u8; 32] {
        &self.key_digest
    }

    pub fn meaning_digest(&self) -> &[u8; 32] {
        &self.meaning_digest
    }

    pub fn result_object_id(&self) -> StoreObjectIdV1 {
        self.result_object_id
    }
}

fn validate_namespace(namespace: &str) -> Result<(), AtomicPublicationError> {
    if namespace.is_empty()
        || namespace.len() > MAX_IDEMPOTENCY_NAMESPACE_BYTES_V1
        || !namespace.is_ascii()
    {
        return Err(AtomicPublicationError::InvalidIdempotencyNamespace);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicGenerationPublicationV1 {
    generation: StoreGenerationV1,
    expected_old: Option<StoreHeadIdV1>,
    objects: Vec<StoreObjectV1>,
    idempotency: StoreIdempotencyV1,
}

impl AtomicGenerationPublicationV1 {
    pub fn new(
        generation: StoreGenerationV1,
        expected_old: Option<StoreHeadIdV1>,
        mut objects: Vec<StoreObjectV1>,
        idempotency: StoreIdempotencyV1,
    ) -> Result<Self, AtomicPublicationError> {
        if objects.is_empty() {
            return Err(AtomicPublicationError::EmptyObjectSet);
        }
        objects.sort_by_key(StoreObjectV1::id);
        if objects.windows(2).any(|pair| pair[0].id() == pair[1].id()) {
            return Err(AtomicPublicationError::DuplicateObject);
        }
        let by_id = objects
            .iter()
            .map(|object| (object.id(), object))
            .collect::<BTreeMap<_, _>>();
        let mut reachable = BTreeSet::new();
        let mut queue = generation.roots().iter().copied().collect::<VecDeque<_>>();
        while let Some(object_id) = queue.pop_front() {
            if !reachable.insert(object_id) {
                continue;
            }
            if let Some(object) = by_id.get(&object_id) {
                queue.extend(object.references().iter().copied());
            }
        }
        if !reachable.contains(&idempotency.result_object_id()) {
            return Err(AtomicPublicationError::ResultNotGenerationRoot);
        }
        Ok(Self {
            generation,
            expected_old,
            objects,
            idempotency,
        })
    }

    pub fn generation(&self) -> &StoreGenerationV1 {
        &self.generation
    }

    pub fn expected_old(&self) -> Option<StoreHeadIdV1> {
        self.expected_old
    }

    pub fn objects(&self) -> &[StoreObjectV1] {
        &self.objects
    }

    pub fn idempotency(&self) -> &StoreIdempotencyV1 {
        &self.idempotency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorePublicationOutcomeV1 {
    Committed {
        head: StoreHeadV1,
        result: StoreObjectV1,
    },
    Replayed {
        head: StoreHeadV1,
        result: StoreObjectV1,
    },
}

impl StorePublicationOutcomeV1 {
    pub fn head(&self) -> &StoreHeadV1 {
        match self {
            Self::Committed { head, .. } | Self::Replayed { head, .. } => head,
        }
    }

    pub fn result(&self) -> &StoreObjectV1 {
        match self {
            Self::Committed { result, .. } | Self::Replayed { result, .. } => result,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AtomicPublicationError {
    #[error("Store idempotency namespace must be non-empty bounded ASCII")]
    InvalidIdempotencyNamespace,
    #[error("atomic Generation publication requires at least one Store Object")]
    EmptyObjectSet,
    #[error("atomic Generation publication repeats a Store Object identity")]
    DuplicateObject,
    #[error("the idempotency result Store Object must be in the exact Generation root closure")]
    ResultNotGenerationRoot,
}
