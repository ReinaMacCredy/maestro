//! Capability and maturity runtime implementation seam owned by Stage 8.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Capability views before Stage 9 source adapters are integrated"
)]

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::capability::generated_catalog::{
    ACTION_TAG_COUNT_V1, CEREMONY_TAG_COUNT_V1,
};
use crate::domain::vnext::capability::literals::InternalJobV1;
use crate::domain::vnext::evidence::{ObservationKindV1, ObservationRecordIdV1, ObservationV1};

const VIEW_DOMAIN_V1: &[u8] = b"maestro.vnext.capability-view.v1";
const MAX_CAPABILITY_FACTS_V1: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CapabilitySubjectV1 {
    Action(u64),
    Ceremony(u64),
    Resource([u8; 32]),
    Tool([u8; 32]),
    Connector([u8; 32]),
    Job(InternalJobV1),
}

impl CapabilitySubjectV1 {
    fn validate(self) -> Result<(), CapabilityViewErrorV1> {
        match self {
            Self::Action(tag) if (1..=ACTION_TAG_COUNT_V1).contains(&tag) => Ok(()),
            Self::Ceremony(tag) if (1..=CEREMONY_TAG_COUNT_V1).contains(&tag) => Ok(()),
            Self::Job(_) => Ok(()),
            Self::Resource(reference) | Self::Tool(reference) | Self::Connector(reference)
                if reference != [0; 32] =>
            {
                Ok(())
            }
            _ => Err(CapabilityViewErrorV1::InvalidSubject),
        }
    }

    fn hash_identity_into(self, hash: &mut Sha256) {
        match self {
            Self::Action(tag) => {
                hash.update([1]);
                hash.update(tag.to_be_bytes());
            }
            Self::Ceremony(tag) => {
                hash.update([2]);
                hash.update(tag.to_be_bytes());
            }
            Self::Resource(reference) => {
                hash.update([3]);
                hash.update(reference);
            }
            Self::Tool(reference) => {
                hash.update([4]);
                hash.update(reference);
            }
            Self::Connector(reference) => {
                hash.update([5]);
                hash.update(reference);
            }
            Self::Job(job) => hash.update([6, job as u8]),
        }
    }

    fn accepts_declared_owner(self, owner: CapabilitySourceOwnerV1) -> bool {
        match self {
            Self::Action(_) | Self::Ceremony(_) => {
                matches!(
                    owner,
                    CapabilitySourceOwnerV1::Contract | CapabilitySourceOwnerV1::CapabilityCatalog
                )
            }
            Self::Resource(_) | Self::Tool(_) | Self::Connector(_) => {
                matches!(
                    owner,
                    CapabilitySourceOwnerV1::Distribution | CapabilitySourceOwnerV1::Installation
                )
            }
            Self::Job(_) => {
                matches!(
                    owner,
                    CapabilitySourceOwnerV1::CapabilityCatalog
                        | CapabilitySourceOwnerV1::Installation
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub(crate) enum CapabilitySourceOwnerV1 {
    Contract = 0,
    Evidence = 1,
    Distribution = 2,
    Installation = 3,
    CapabilityCatalog = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum CapabilityAvailabilityV1 {
    DeclaredSupport = 0,
    ObservedAvailable = 1,
    ObservedUnavailable = 2,
    Unknown = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum CapabilityProbeStatusV1 {
    NotRequired = 0,
    ObservationConsumed = 1,
    RequiredButNotRun = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilitySourceFactV1 {
    snapshot_ref: [u8; 32],
    subject: CapabilitySubjectV1,
    owner: CapabilitySourceOwnerV1,
    source_ref: [u8; 32],
    source_revision_ref: [u8; 32],
    declared_support: bool,
    observation_ref: Option<ObservationRecordIdV1>,
    observed_available: Option<bool>,
}

impl CapabilitySourceFactV1 {
    pub(crate) fn declared(
        snapshot_ref: [u8; 32],
        subject: CapabilitySubjectV1,
        owner: CapabilitySourceOwnerV1,
        source_ref: [u8; 32],
        source_revision_ref: [u8; 32],
        declared_support: bool,
    ) -> Result<Self, CapabilityViewErrorV1> {
        subject.validate()?;
        require_nonzero(snapshot_ref)?;
        require_nonzero(source_ref)?;
        require_nonzero(source_revision_ref)?;
        if !subject.accepts_declared_owner(owner) {
            return Err(CapabilityViewErrorV1::InvalidSourceOwner);
        }
        Ok(Self {
            snapshot_ref,
            subject,
            owner,
            source_ref,
            source_revision_ref,
            declared_support,
            observation_ref: None,
            observed_available: None,
        })
    }

    pub(crate) fn observed_probe(
        snapshot_ref: [u8; 32],
        subject: CapabilitySubjectV1,
        source_ref: [u8; 32],
        source_revision_ref: [u8; 32],
        declared_support: bool,
        observation: &ObservationV1,
        observed_available: bool,
    ) -> Result<Self, CapabilityViewErrorV1> {
        if observation.kind() != ObservationKindV1::CapabilityProbe {
            return Err(CapabilityViewErrorV1::WrongObservationKind);
        }
        subject.validate()?;
        require_nonzero(snapshot_ref)?;
        require_nonzero(source_ref)?;
        require_nonzero(source_revision_ref)?;
        Ok(Self {
            snapshot_ref,
            subject,
            owner: CapabilitySourceOwnerV1::Evidence,
            source_ref,
            source_revision_ref,
            declared_support,
            observation_ref: Some(observation.id()),
            observed_available: Some(observed_available),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilitySourceBindingV1 {
    source_owner: CapabilitySourceOwnerV1,
    source_ref: [u8; 32],
    source_revision_ref: [u8; 32],
    observation_ref: Option<ObservationRecordIdV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilityEntryV1 {
    subject: CapabilitySubjectV1,
    availability: CapabilityAvailabilityV1,
    probe_status: CapabilityProbeStatusV1,
    sources: Vec<CapabilitySourceBindingV1>,
}

impl CapabilityEntryV1 {
    pub(crate) const fn subject(&self) -> CapabilitySubjectV1 {
        self.subject
    }

    pub(crate) const fn availability(&self) -> CapabilityAvailabilityV1 {
        self.availability
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilityViewV1 {
    snapshot_ref: [u8; 32],
    source_closure_ref: [u8; 32],
    entries: Vec<CapabilityEntryV1>,
}

impl CapabilityViewV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) const fn source_closure_ref(&self) -> [u8; 32] {
        self.source_closure_ref
    }

    pub(crate) fn entries(&self) -> &[CapabilityEntryV1] {
        &self.entries
    }

    pub(crate) fn has_unknowns(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.availability == CapabilityAvailabilityV1::Unknown)
    }
}

pub(crate) fn build_capability_view(
    snapshot_ref: [u8; 32],
    facts: impl IntoIterator<Item = CapabilitySourceFactV1>,
) -> Result<CapabilityViewV1, CapabilityViewErrorV1> {
    require_nonzero(snapshot_ref)?;
    let facts = facts.into_iter().collect::<Vec<_>>();
    if facts.len() > MAX_CAPABILITY_FACTS_V1 {
        return Err(CapabilityViewErrorV1::BoundExceeded);
    }
    if facts.iter().any(|fact| fact.snapshot_ref != snapshot_ref) {
        return Err(CapabilityViewErrorV1::MixedSnapshot);
    }
    let mut grouped = BTreeMap::<CapabilitySubjectV1, Vec<CapabilitySourceFactV1>>::new();
    for fact in facts {
        fact.subject.validate()?;
        grouped.entry(fact.subject).or_default().push(fact);
    }
    let mut entries = Vec::with_capacity(grouped.len());
    for (subject, mut subject_facts) in grouped {
        subject_facts.sort_by_key(|fact| {
            (
                fact.owner,
                fact.source_ref,
                fact.source_revision_ref,
                fact.observation_ref,
            )
        });
        if subject_facts.windows(2).any(|pair| {
            pair[0].owner == pair[1].owner
                && pair[0].source_ref == pair[1].source_ref
                && pair[0].source_revision_ref == pair[1].source_revision_ref
                && pair[0].observation_ref == pair[1].observation_ref
        }) {
            return Err(CapabilityViewErrorV1::DuplicateSource);
        }
        let observed_states = subject_facts
            .iter()
            .filter_map(|fact| fact.observed_available)
            .collect::<Vec<_>>();
        let observed_conflict = observed_states
            .first()
            .is_some_and(|first| observed_states.iter().any(|state| state != first));
        let declared_conflict = subject_facts.first().is_some_and(|first| {
            subject_facts
                .iter()
                .any(|fact| fact.declared_support != first.declared_support)
        });
        let (availability, probe_status) = if observed_conflict || declared_conflict {
            (
                CapabilityAvailabilityV1::Unknown,
                CapabilityProbeStatusV1::RequiredButNotRun,
            )
        } else if let Some(observed) = observed_states.first() {
            (
                if *observed {
                    CapabilityAvailabilityV1::ObservedAvailable
                } else {
                    CapabilityAvailabilityV1::ObservedUnavailable
                },
                CapabilityProbeStatusV1::ObservationConsumed,
            )
        } else if subject_facts
            .first()
            .is_some_and(|fact| fact.declared_support)
        {
            (
                CapabilityAvailabilityV1::DeclaredSupport,
                CapabilityProbeStatusV1::NotRequired,
            )
        } else {
            (
                CapabilityAvailabilityV1::Unknown,
                CapabilityProbeStatusV1::RequiredButNotRun,
            )
        };
        let sources = subject_facts
            .into_iter()
            .map(|fact| CapabilitySourceBindingV1 {
                source_owner: fact.owner,
                source_ref: fact.source_ref,
                source_revision_ref: fact.source_revision_ref,
                observation_ref: fact.observation_ref,
            })
            .collect();
        entries.push(CapabilityEntryV1 {
            subject,
            availability,
            probe_status,
            sources,
        });
    }
    let source_closure_ref = capability_view_identity(snapshot_ref, &entries);
    Ok(CapabilityViewV1 {
        snapshot_ref,
        source_closure_ref,
        entries,
    })
}

fn capability_view_identity(snapshot_ref: [u8; 32], entries: &[CapabilityEntryV1]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(VIEW_DOMAIN_V1);
    hash.update(snapshot_ref);
    hash.update((entries.len() as u64).to_be_bytes());
    for entry in entries {
        entry.subject.hash_identity_into(&mut hash);
        hash.update([entry.availability as u8]);
        hash.update([entry.probe_status as u8]);
        hash.update((entry.sources.len() as u64).to_be_bytes());
        for source in &entry.sources {
            hash.update([source.source_owner as u8]);
            hash.update(source.source_ref);
            hash.update(source.source_revision_ref);
            hash.update(source.observation_ref.map_or([0; 32], |id| *id.as_bytes()));
        }
    }
    hash.finalize().into()
}

fn require_nonzero(value: [u8; 32]) -> Result<(), CapabilityViewErrorV1> {
    if value == [0; 32] {
        return Err(CapabilityViewErrorV1::InvalidReference);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CapabilityViewErrorV1 {
    #[error("Capability reference is invalid")]
    InvalidReference,
    #[error("Capability subject is invalid")]
    InvalidSubject,
    #[error("Capability source owner cannot establish this subject")]
    InvalidSourceOwner,
    #[error("Capability facts cross coherent snapshots")]
    MixedSnapshot,
    #[error("Capability fact bound is exceeded")]
    BoundExceeded,
    #[error("Capability source fact is duplicated")]
    DuplicateSource,
    #[error("Capability probe Observation kind is not CapabilityProbe")]
    WrongObservationKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn identity_hashed_discriminants_are_pinned() {
        assert_eq!(CapabilitySourceOwnerV1::Contract as u8, 0);
        assert_eq!(CapabilitySourceOwnerV1::CapabilityCatalog as u8, 4);
        assert_eq!(CapabilityAvailabilityV1::DeclaredSupport as u8, 0);
        assert_eq!(CapabilityAvailabilityV1::Unknown as u8, 3);
        assert_eq!(CapabilityProbeStatusV1::NotRequired as u8, 0);
        assert_eq!(CapabilityProbeStatusV1::RequiredButNotRun as u8, 2);
    }

    #[test]
    fn subject_tag_bounds_follow_the_generated_catalog_counts() {
        assert!(
            CapabilitySubjectV1::Action(ACTION_TAG_COUNT_V1)
                .validate()
                .is_ok()
        );
        assert_eq!(
            CapabilitySubjectV1::Action(ACTION_TAG_COUNT_V1 + 1).validate(),
            Err(CapabilityViewErrorV1::InvalidSubject)
        );
        assert!(
            CapabilitySubjectV1::Ceremony(CEREMONY_TAG_COUNT_V1)
                .validate()
                .is_ok()
        );
        assert_eq!(
            CapabilitySubjectV1::Ceremony(CEREMONY_TAG_COUNT_V1 + 1).validate(),
            Err(CapabilityViewErrorV1::InvalidSubject)
        );
        assert_eq!(
            CapabilitySubjectV1::Action(0).validate(),
            Err(CapabilityViewErrorV1::InvalidSubject)
        );
    }

    #[test]
    fn declared_support_does_not_become_permission_or_applicability() {
        let view = build_capability_view(
            digest(1),
            [CapabilitySourceFactV1::declared(
                digest(1),
                CapabilitySubjectV1::Action(1),
                CapabilitySourceOwnerV1::Contract,
                digest(2),
                digest(3),
                true,
            )
            .unwrap()],
        )
        .unwrap();
        assert_eq!(
            view.entries()[0].availability(),
            CapabilityAvailabilityV1::DeclaredSupport
        );
        assert_eq!(
            view.entries()[0].probe_status,
            CapabilityProbeStatusV1::NotRequired
        );
    }

    #[test]
    fn mixed_snapshots_fail_closed() {
        let error = build_capability_view(
            digest(1),
            [CapabilitySourceFactV1::declared(
                digest(9),
                CapabilitySubjectV1::Ceremony(1),
                CapabilitySourceOwnerV1::Contract,
                digest(2),
                digest(3),
                true,
            )
            .unwrap()],
        )
        .unwrap_err();
        assert_eq!(error, CapabilityViewErrorV1::MixedSnapshot);
    }

    #[test]
    fn conflicting_source_facts_are_unknown_not_ranked() {
        let first = CapabilitySourceFactV1::declared(
            digest(1),
            CapabilitySubjectV1::Tool(digest(2)),
            CapabilitySourceOwnerV1::Installation,
            digest(3),
            digest(4),
            true,
        )
        .unwrap();
        let second = CapabilitySourceFactV1::declared(
            digest(1),
            CapabilitySubjectV1::Tool(digest(2)),
            CapabilitySourceOwnerV1::Distribution,
            digest(5),
            digest(6),
            false,
        )
        .unwrap();
        let view = build_capability_view(digest(1), [first, second]).unwrap();
        assert_eq!(
            view.entries()[0].availability(),
            CapabilityAvailabilityV1::Unknown
        );
        assert!(view.has_unknowns());
    }
}
