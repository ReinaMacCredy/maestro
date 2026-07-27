//! Capability maturity and assessment implementation seam.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Maturity views before Stage 9 source adapters are integrated"
)]

use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::vnext::capability::runtime::CapabilityViewV1;

const MAX_FACTS_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MaturityAxisV1 {
    CapabilityCoverage,
    SchedulingStance,
    ProofCoverage,
    Liveness,
    Security,
    Budget,
    OperatingLimit,
    KillSwitch,
}

impl MaturityAxisV1 {
    pub(crate) const ALL: [Self; 8] = [
        Self::CapabilityCoverage,
        Self::SchedulingStance,
        Self::ProofCoverage,
        Self::Liveness,
        Self::Security,
        Self::Budget,
        Self::OperatingLimit,
        Self::KillSwitch,
    ];

    const fn accepts_owner(self, owner: MaturitySourceOwnerV1) -> bool {
        match self {
            Self::CapabilityCoverage => matches!(
                owner,
                MaturitySourceOwnerV1::Contract
                    | MaturitySourceOwnerV1::Evidence
                    | MaturitySourceOwnerV1::Distribution
                    | MaturitySourceOwnerV1::Installation
            ),
            Self::SchedulingStance => matches!(owner, MaturitySourceOwnerV1::Planning),
            Self::ProofCoverage => matches!(owner, MaturitySourceOwnerV1::Evidence),
            Self::Liveness => matches!(
                owner,
                MaturitySourceOwnerV1::Evidence
                    | MaturitySourceOwnerV1::Execution
                    | MaturitySourceOwnerV1::Installation
            ),
            Self::Security => matches!(
                owner,
                MaturitySourceOwnerV1::Evidence | MaturitySourceOwnerV1::Authority
            ),
            Self::Budget => matches!(
                owner,
                MaturitySourceOwnerV1::Authority | MaturitySourceOwnerV1::Execution
            ),
            Self::OperatingLimit => matches!(
                owner,
                MaturitySourceOwnerV1::Contract
                    | MaturitySourceOwnerV1::Execution
                    | MaturitySourceOwnerV1::Distribution
                    | MaturitySourceOwnerV1::Installation
            ),
            Self::KillSwitch => matches!(owner, MaturitySourceOwnerV1::Authority),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaturityLevelV1 {
    Established,
    Partial,
    Blocked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaturitySourceOwnerV1 {
    Contract,
    Evidence,
    Authority,
    Planning,
    Execution,
    Distribution,
    Installation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MaturityFactV1 {
    snapshot_ref: [u8; 32],
    axis: MaturityAxisV1,
    level: MaturityLevelV1,
    source_owner: MaturitySourceOwnerV1,
    source_ref: [u8; 32],
    source_revision_ref: [u8; 32],
    bounded_reason_ref: [u8; 32],
}

impl MaturityFactV1 {
    pub(crate) fn new(
        snapshot_ref: [u8; 32],
        axis: MaturityAxisV1,
        level: MaturityLevelV1,
        source_owner: MaturitySourceOwnerV1,
        source_ref: [u8; 32],
        source_revision_ref: [u8; 32],
        bounded_reason_ref: [u8; 32],
    ) -> Result<Self, MaturityErrorV1> {
        require_nonzero(snapshot_ref)?;
        require_nonzero(source_ref)?;
        require_nonzero(source_revision_ref)?;
        require_nonzero(bounded_reason_ref)?;
        if !axis.accepts_owner(source_owner) {
            return Err(MaturityErrorV1::InvalidSourceOwner);
        }
        Ok(Self {
            snapshot_ref,
            axis,
            level,
            source_owner,
            source_ref,
            source_revision_ref,
            bounded_reason_ref,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MaturityAxisViewV1 {
    axis: MaturityAxisV1,
    level: MaturityLevelV1,
    source_refs: Vec<[u8; 32]>,
    bounded_reason_refs: Vec<[u8; 32]>,
}

impl MaturityAxisViewV1 {
    pub(crate) const fn axis(&self) -> MaturityAxisV1 {
        self.axis
    }

    pub(crate) const fn level(&self) -> MaturityLevelV1 {
        self.level
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MaturityViewV1 {
    snapshot_ref: [u8; 32],
    capability_source_closure_ref: [u8; 32],
    axes: Vec<MaturityAxisViewV1>,
}

impl MaturityViewV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.snapshot_ref
    }

    pub(crate) const fn capability_source_closure_ref(&self) -> [u8; 32] {
        self.capability_source_closure_ref
    }

    pub(crate) fn axes(&self) -> &[MaturityAxisViewV1] {
        &self.axes
    }
}

pub(crate) fn build_maturity_view(
    snapshot_ref: [u8; 32],
    capability: &CapabilityViewV1,
    facts: impl IntoIterator<Item = MaturityFactV1>,
) -> Result<MaturityViewV1, MaturityErrorV1> {
    require_nonzero(snapshot_ref)?;
    if capability.snapshot_ref() != snapshot_ref {
        return Err(MaturityErrorV1::MixedSnapshot);
    }
    let facts = facts.into_iter().collect::<Vec<_>>();
    if facts.len() > MAX_FACTS_V1 {
        return Err(MaturityErrorV1::BoundExceeded);
    }
    if facts.iter().any(|fact| fact.snapshot_ref != snapshot_ref) {
        return Err(MaturityErrorV1::MixedSnapshot);
    }
    let mut grouped = BTreeMap::<MaturityAxisV1, Vec<MaturityFactV1>>::new();
    for fact in facts {
        grouped.entry(fact.axis).or_default().push(fact);
    }
    let axes = MaturityAxisV1::ALL
        .into_iter()
        .map(|axis| {
            let mut facts = grouped.remove(&axis).unwrap_or_default();
            facts.sort_by_key(|fact| {
                (
                    fact.source_owner as u8,
                    fact.source_ref,
                    fact.source_revision_ref,
                )
            });
            if facts.windows(2).any(|pair| {
                pair[0].source_owner == pair[1].source_owner
                    && pair[0].source_ref == pair[1].source_ref
                    && pair[0].source_revision_ref == pair[1].source_revision_ref
            }) {
                return Err(MaturityErrorV1::DuplicateSource);
            }
            let conflict = facts
                .first()
                .is_some_and(|first| facts.iter().any(|fact| fact.level != first.level));
            let mut source_refs = facts.iter().map(|fact| fact.source_ref).collect::<Vec<_>>();
            source_refs.sort();
            source_refs.dedup();
            let mut bounded_reason_refs = facts
                .iter()
                .map(|fact| fact.bounded_reason_ref)
                .collect::<Vec<_>>();
            bounded_reason_refs.sort();
            bounded_reason_refs.dedup();
            let capability_unknown =
                axis == MaturityAxisV1::CapabilityCoverage && capability.has_unknowns();
            let level = if capability_unknown || conflict || facts.is_empty() {
                MaturityLevelV1::Unknown
            } else {
                facts[0].level
            };
            Ok(MaturityAxisViewV1 {
                axis,
                level,
                source_refs,
                bounded_reason_refs,
            })
        })
        .collect::<Result<Vec<_>, MaturityErrorV1>>()?;
    Ok(MaturityViewV1 {
        snapshot_ref,
        capability_source_closure_ref: capability.source_closure_ref(),
        axes,
    })
}

fn require_nonzero(value: [u8; 32]) -> Result<(), MaturityErrorV1> {
    if value == [0; 32] {
        return Err(MaturityErrorV1::InvalidReference);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum MaturityErrorV1 {
    #[error("Maturity reference is invalid")]
    InvalidReference,
    #[error("Maturity facts cross coherent snapshots")]
    MixedSnapshot,
    #[error("Maturity source owner cannot establish this axis")]
    InvalidSourceOwner,
    #[error("Maturity fact bound is exceeded")]
    BoundExceeded,
    #[error("Maturity source is duplicated within one axis")]
    DuplicateSource,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::capability::runtime::{
        CapabilitySourceFactV1, CapabilitySourceOwnerV1, CapabilitySubjectV1, build_capability_view,
    };

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn capability() -> CapabilityViewV1 {
        build_capability_view(
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
        .unwrap()
    }

    #[test]
    fn maturity_is_multidimensional_and_has_no_scalar_permission() {
        let capability = capability();
        let view = build_maturity_view(
            digest(1),
            &capability,
            [MaturityFactV1::new(
                digest(1),
                MaturityAxisV1::Security,
                MaturityLevelV1::Established,
                MaturitySourceOwnerV1::Evidence,
                digest(4),
                digest(5),
                digest(6),
            )
            .unwrap()],
        )
        .unwrap();
        assert_eq!(view.axes().len(), MaturityAxisV1::ALL.len());
        assert_eq!(
            view.axes()
                .iter()
                .find(|axis| axis.axis() == MaturityAxisV1::Security)
                .unwrap()
                .level(),
            MaturityLevelV1::Established
        );
        assert_eq!(
            view.axes()
                .iter()
                .find(|axis| axis.axis() == MaturityAxisV1::ProofCoverage)
                .unwrap()
                .level(),
            MaturityLevelV1::Unknown
        );
    }

    #[test]
    fn duplicate_maturity_sources_within_one_axis_are_refused() {
        let capability = capability();
        let fact = |level| {
            MaturityFactV1::new(
                digest(1),
                MaturityAxisV1::Security,
                level,
                MaturitySourceOwnerV1::Evidence,
                digest(4),
                digest(5),
                digest(6),
            )
            .unwrap()
        };
        let error = build_maturity_view(
            digest(1),
            &capability,
            [
                fact(MaturityLevelV1::Established),
                fact(MaturityLevelV1::Partial),
            ],
        )
        .unwrap_err();
        assert_eq!(error, MaturityErrorV1::DuplicateSource);
    }

    #[test]
    fn maturity_rejects_mixed_snapshot_inputs() {
        let capability = capability();
        let error = build_maturity_view(digest(9), &capability, []).unwrap_err();
        assert_eq!(error, MaturityErrorV1::MixedSnapshot);
    }

    #[test]
    fn kill_switch_cannot_be_inferred_from_non_authority_sources() {
        let error = MaturityFactV1::new(
            digest(1),
            MaturityAxisV1::KillSwitch,
            MaturityLevelV1::Established,
            MaturitySourceOwnerV1::Evidence,
            digest(2),
            digest(3),
            digest(4),
        )
        .unwrap_err();
        assert_eq!(error, MaturityErrorV1::InvalidSourceOwner);
    }
}
