//! Observation-facing diagnostics implementation seam.

#![expect(
    dead_code,
    reason = "Stage 8 freezes diagnostics before the Stage 10 presentation adapter is integrated"
)]

use thiserror::Error;

use crate::domain::vnext::authority::ProtectedContinuityDiagnosticReleasedEnvelopeV1;
use crate::domain::vnext::evidence::{ObservationKindV1, ObservationRecordIdV1};

const MAX_ORDINARY_DIAGNOSTICS_V1: usize = 512;
const MAX_BOUNDED_REASON_REFS_V1: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrdinaryDiagnosticClassV1 {
    Current,
    Stale,
    Indeterminate,
    CoarseUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OrdinaryDiagnosticFactV1 {
    snapshot_ref: [u8; 32],
    observation_ref: ObservationRecordIdV1,
    observation_kind: ObservationKindV1,
    subject_commitment: [u8; 32],
    class: OrdinaryDiagnosticClassV1,
    bounded_reason_refs: Vec<[u8; 32]>,
}

impl OrdinaryDiagnosticFactV1 {
    pub(crate) fn new(
        snapshot_ref: [u8; 32],
        observation_ref: ObservationRecordIdV1,
        observation_kind: ObservationKindV1,
        subject_commitment: [u8; 32],
        class: OrdinaryDiagnosticClassV1,
        mut bounded_reason_refs: Vec<[u8; 32]>,
    ) -> Result<Self, DiagnosticErrorV1> {
        require_nonzero(snapshot_ref)?;
        require_nonzero(subject_commitment)?;
        let protected = observation_kind == ObservationKindV1::ProtectedContinuityDiagnostic;
        if protected {
            bounded_reason_refs.clear();
        } else if bounded_reason_refs.len() > MAX_BOUNDED_REASON_REFS_V1
            || bounded_reason_refs.contains(&[0; 32])
        {
            return Err(DiagnosticErrorV1::Unavailable);
        }
        bounded_reason_refs.sort();
        bounded_reason_refs.dedup();
        Ok(Self {
            snapshot_ref,
            observation_ref,
            observation_kind,
            subject_commitment,
            class: if protected {
                OrdinaryDiagnosticClassV1::CoarseUnavailable
            } else {
                class
            },
            bounded_reason_refs,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OrdinaryDiagnosticEntryV1 {
    observation_ref: ObservationRecordIdV1,
    observation_kind: ObservationKindV1,
    class: OrdinaryDiagnosticClassV1,
    bounded_reason_refs: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OrdinaryDiagnosticViewV1 {
    snapshot_ref: [u8; 32],
    entries: Vec<OrdinaryDiagnosticEntryV1>,
}

impl OrdinaryDiagnosticViewV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) fn entries(&self) -> &[OrdinaryDiagnosticEntryV1] {
        &self.entries
    }
}

pub(crate) fn project_ordinary_diagnostics(
    snapshot_ref: [u8; 32],
    requested_subject_commitment: [u8; 32],
    facts: impl IntoIterator<Item = OrdinaryDiagnosticFactV1>,
) -> Result<OrdinaryDiagnosticViewV1, DiagnosticErrorV1> {
    require_nonzero(snapshot_ref)?;
    require_nonzero(requested_subject_commitment)?;
    let facts = facts.into_iter().collect::<Vec<_>>();
    if facts.len() > MAX_ORDINARY_DIAGNOSTICS_V1
        || facts.iter().any(|fact| {
            fact.snapshot_ref != snapshot_ref
                || fact.subject_commitment != requested_subject_commitment
        })
    {
        return Err(DiagnosticErrorV1::Unavailable);
    }
    let mut entries = facts
        .into_iter()
        .map(|fact| OrdinaryDiagnosticEntryV1 {
            observation_ref: fact.observation_ref,
            observation_kind: fact.observation_kind,
            class: fact.class,
            bounded_reason_refs: if fact.class == OrdinaryDiagnosticClassV1::CoarseUnavailable {
                Vec::new()
            } else {
                fact.bounded_reason_refs
            },
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| *entry.observation_ref.as_bytes());
    if entries
        .windows(2)
        .any(|pair| pair[0].observation_ref == pair[1].observation_ref)
    {
        return Err(DiagnosticErrorV1::Unavailable);
    }
    Ok(OrdinaryDiagnosticViewV1 {
        snapshot_ref,
        entries,
    })
}

pub(crate) struct ProtectedDiagnosticEnvelopeV1 {
    bytes: Box<[u8]>,
}

impl ProtectedDiagnosticEnvelopeV1 {
    pub(crate) fn from_authority_release(
        released: ProtectedContinuityDiagnosticReleasedEnvelopeV1,
    ) -> Self {
        Self {
            bytes: released.into_bytes(),
        }
    }

    pub(crate) fn into_bytes(self) -> Box<[u8]> {
        self.bytes
    }
}

fn require_nonzero(value: [u8; 32]) -> Result<(), DiagnosticErrorV1> {
    if value == [0; 32] {
        return Err(DiagnosticErrorV1::Unavailable);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum DiagnosticErrorV1 {
    #[error("diagnostic unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn protected_observation_is_coarse_on_the_ordinary_path() {
        let fact = OrdinaryDiagnosticFactV1::new(
            digest(1),
            ObservationRecordIdV1::from_bytes(digest(2)).unwrap(),
            ObservationKindV1::ProtectedContinuityDiagnostic,
            digest(3),
            OrdinaryDiagnosticClassV1::Current,
            vec![digest(4)],
        )
        .unwrap();
        assert!(fact.bounded_reason_refs.is_empty());
        let view = project_ordinary_diagnostics(digest(1), digest(3), [fact]).unwrap();
        assert_eq!(
            view.entries()[0].class,
            OrdinaryDiagnosticClassV1::CoarseUnavailable
        );
        assert!(view.entries()[0].bounded_reason_refs.is_empty());
    }

    #[test]
    fn subject_and_snapshot_mismatch_share_one_bounded_refusal() {
        let fact = OrdinaryDiagnosticFactV1::new(
            digest(1),
            ObservationRecordIdV1::from_bytes(digest(2)).unwrap(),
            ObservationKindV1::ProcessLiveness,
            digest(3),
            OrdinaryDiagnosticClassV1::Current,
            vec![],
        )
        .unwrap();
        assert_eq!(
            project_ordinary_diagnostics(digest(9), digest(3), [fact.clone()]).unwrap_err(),
            DiagnosticErrorV1::Unavailable
        );
        assert_eq!(
            project_ordinary_diagnostics(digest(1), digest(9), [fact]).unwrap_err(),
            DiagnosticErrorV1::Unavailable
        );
    }

    #[test]
    fn protected_adapter_requires_move_consumption() {
        let adapter: fn(
            ProtectedContinuityDiagnosticReleasedEnvelopeV1,
        ) -> ProtectedDiagnosticEnvelopeV1 = ProtectedDiagnosticEnvelopeV1::from_authority_release;
        let consumer: fn(ProtectedDiagnosticEnvelopeV1) -> Box<[u8]> =
            ProtectedDiagnosticEnvelopeV1::into_bytes;
        let _ = (adapter, consumer);
    }

    #[test]
    fn duplicate_observation_facts_are_refused() {
        let observation_ref = ObservationRecordIdV1::from_bytes(digest(2)).unwrap();
        let current = OrdinaryDiagnosticFactV1::new(
            digest(1),
            observation_ref,
            ObservationKindV1::ProcessLiveness,
            digest(3),
            OrdinaryDiagnosticClassV1::Current,
            vec![],
        )
        .unwrap();
        let stale = OrdinaryDiagnosticFactV1::new(
            digest(1),
            observation_ref,
            ObservationKindV1::ProcessLiveness,
            digest(3),
            OrdinaryDiagnosticClassV1::Stale,
            vec![digest(4)],
        )
        .unwrap();
        assert_eq!(
            project_ordinary_diagnostics(digest(1), digest(3), [current, stale]).unwrap_err(),
            DiagnosticErrorV1::Unavailable
        );
    }
}
