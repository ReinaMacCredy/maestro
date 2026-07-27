use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::domain::identity::{StoreHeadIdV1, StoreObjectIdV1};

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
        let reachable = generation_object_reachability(&generation, &objects)?;
        if !reachable.contains(&idempotency.result_object_id()) {
            return Err(AtomicPublicationError::ResultNotGenerationRoot);
        }
        if objects
            .iter()
            .any(|object| !reachable.contains(&object.id()))
        {
            return Err(AtomicPublicationError::ObjectOutsideGeneration);
        }
        Ok(Self {
            generation,
            expected_old,
            objects,
            idempotency,
        })
    }

    pub(crate) fn new_from_object_superset(
        generation: StoreGenerationV1,
        expected_old: Option<StoreHeadIdV1>,
        objects: Vec<StoreObjectV1>,
        idempotency: StoreIdempotencyV1,
    ) -> Result<Self, AtomicPublicationError> {
        let objects = exact_generation_object_closure(&generation, objects)?;
        Self::new(generation, expected_old, objects, idempotency)
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

fn exact_generation_object_closure(
    generation: &StoreGenerationV1,
    mut objects: Vec<StoreObjectV1>,
) -> Result<Vec<StoreObjectV1>, AtomicPublicationError> {
    objects.sort_by_key(StoreObjectV1::id);
    if objects.windows(2).any(|pair| pair[0].id() == pair[1].id()) {
        return Err(AtomicPublicationError::DuplicateObject);
    }
    let reachable = generation_object_reachability(generation, &objects)?;
    objects.retain(|object| reachable.contains(&object.id()));
    Ok(objects)
}

fn generation_object_reachability(
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
) -> Result<BTreeSet<StoreObjectIdV1>, AtomicPublicationError> {
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
        let object = by_id
            .get(&object_id)
            .ok_or(AtomicPublicationError::MissingGenerationObject(object_id))?;
        queue.extend(object.references().iter().copied());
    }
    Ok(reachable)
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
    #[error(
        "atomic Generation publication supplied a Store Object outside the exact Generation closure"
    )]
    ObjectOutsideGeneration,
    #[error("atomic Generation closure is missing referenced Store Object {0}")]
    MissingGenerationObject(StoreObjectIdV1),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::domain::persistence::{StoreCompatibilityV1, StoreDomainV1, StoreRoleV1};
    use crate::foundation::core::deterministic_cbor::CborValue;

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    #[test]
    fn atomic_publication_rejects_unreachable_supplied_objects() {
        let schema = SchemaIdV1::parse(&rendered(10)).unwrap();
        let root = StoreObjectV1::new(schema, CborValue::Unsigned(1), vec![]).unwrap();
        let unreachable = StoreObjectV1::new(schema, CborValue::Unsigned(2), vec![]).unwrap();
        let generation = StoreGenerationV1::new(
            StoreDomainV1::derive(StoreRoleV1::Repository, b"stage5-atomic-publication").unwrap(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(11)).unwrap(),
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![root.id()],
        )
        .unwrap();
        let idempotency =
            StoreIdempotencyV1::new("stage5-unreachable-object", [12; 32], [13; 32], root.id())
                .unwrap();
        assert_eq!(
            AtomicGenerationPublicationV1::new(
                generation,
                None,
                vec![root, unreachable],
                idempotency,
            )
            .unwrap_err(),
            AtomicPublicationError::ObjectOutsideGeneration
        );
    }

    #[test]
    fn publication_builder_reduces_a_superset_to_the_exact_generation_closure() {
        let schema = SchemaIdV1::parse(&rendered(20)).unwrap();
        let root = StoreObjectV1::new(schema, CborValue::Unsigned(1), vec![]).unwrap();
        let superseded = StoreObjectV1::new(schema, CborValue::Unsigned(2), vec![]).unwrap();
        let generation = StoreGenerationV1::new(
            StoreDomainV1::derive(StoreRoleV1::Repository, b"stage5-builder-closure").unwrap(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(21)).unwrap(),
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![root.id()],
        )
        .unwrap();
        let idempotency =
            StoreIdempotencyV1::new("stage5-builder-closure", [22; 32], [23; 32], root.id())
                .unwrap();
        let publication = AtomicGenerationPublicationV1::new_from_object_superset(
            generation,
            None,
            vec![root.clone(), superseded],
            idempotency,
        )
        .unwrap();
        assert_eq!(publication.objects(), &[root]);
    }

    #[test]
    fn generation_closure_rejects_a_missing_referenced_object() {
        let schema = SchemaIdV1::parse(&rendered(30)).unwrap();
        let missing = StoreObjectIdV1::parse(&rendered(31)).unwrap();
        let root = StoreObjectV1::new(schema, CborValue::Unsigned(1), vec![missing]).unwrap();
        let generation = StoreGenerationV1::new(
            StoreDomainV1::derive(StoreRoleV1::Repository, b"stage5-missing-closure").unwrap(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(32)).unwrap(),
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![root.id()],
        )
        .unwrap();
        let idempotency =
            StoreIdempotencyV1::new("stage5-missing-closure", [33; 32], [34; 32], root.id())
                .unwrap();
        assert_eq!(
            AtomicGenerationPublicationV1::new(generation, None, vec![root], idempotency)
                .unwrap_err(),
            AtomicPublicationError::MissingGenerationObject(missing)
        );
    }
}
