use thiserror::Error;

use super::catalog::ContinuityReferenceV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum HTimeAcceptanceRelationV1 {
    ContextGenesis = 1,
    Same = 2,
    Advance = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HTimeCarryBasisV1 {
    ExactNoLineageChange,
    CompleteCarryMapping { mapping: ContinuityReferenceV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HTimeContinuationContributionV1 {
    CarryOnly,
    CarryPlusFreshLower { lower_bound: u64, upper_bound: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedAuthorityTimeFloorV1 {
    stable_lineage: ContinuityReferenceV1,
    coordinate: ContinuityReferenceV1,
    policy_stack: ContinuityReferenceV1,
    lower_bound: u64,
    relation: HTimeAcceptanceRelationV1,
    carry_basis: HTimeCarryBasisV1,
}

impl AcceptedAuthorityTimeFloorV1 {
    pub fn context_genesis(
        stable_lineage: ContinuityReferenceV1,
        coordinate: ContinuityReferenceV1,
        policy_stack: ContinuityReferenceV1,
        origin_commitment: ContinuityReferenceV1,
        fresh_lower_bound: u64,
        fresh_upper_bound: u64,
    ) -> Result<Self, HTimeAcceptanceErrorV1> {
        if fresh_lower_bound > fresh_upper_bound {
            return Err(HTimeAcceptanceErrorV1::InvalidFreshBounds);
        }
        Ok(Self {
            stable_lineage,
            coordinate,
            policy_stack,
            lower_bound: fresh_lower_bound,
            relation: HTimeAcceptanceRelationV1::ContextGenesis,
            carry_basis: HTimeCarryBasisV1::CompleteCarryMapping {
                mapping: origin_commitment,
            },
        })
    }

    pub fn continue_from(
        prior: &Self,
        target_stable_lineage: ContinuityReferenceV1,
        target_coordinate: ContinuityReferenceV1,
        target_policy_stack: ContinuityReferenceV1,
        carry_basis: HTimeCarryBasisV1,
        contribution: HTimeContinuationContributionV1,
    ) -> Result<Self, HTimeAcceptanceErrorV1> {
        match carry_basis {
            HTimeCarryBasisV1::ExactNoLineageChange
                if target_stable_lineage != prior.stable_lineage
                    || target_coordinate != prior.coordinate
                    || target_policy_stack != prior.policy_stack =>
            {
                return Err(HTimeAcceptanceErrorV1::FalseNoLineageChange);
            }
            HTimeCarryBasisV1::ExactNoLineageChange
            | HTimeCarryBasisV1::CompleteCarryMapping { .. } => {}
        }

        let lower_bound = match contribution {
            HTimeContinuationContributionV1::CarryOnly => prior.lower_bound,
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound,
                upper_bound,
            } => {
                if lower_bound > upper_bound || upper_bound < prior.lower_bound {
                    return Err(HTimeAcceptanceErrorV1::InvalidFreshBounds);
                }
                prior.lower_bound.max(lower_bound)
            }
        };
        let relation = if lower_bound == prior.lower_bound {
            HTimeAcceptanceRelationV1::Same
        } else {
            HTimeAcceptanceRelationV1::Advance
        };
        Ok(Self {
            stable_lineage: target_stable_lineage,
            coordinate: target_coordinate,
            policy_stack: target_policy_stack,
            lower_bound,
            relation,
            carry_basis,
        })
    }

    pub const fn stable_lineage(&self) -> ContinuityReferenceV1 {
        self.stable_lineage
    }

    pub const fn coordinate(&self) -> ContinuityReferenceV1 {
        self.coordinate
    }

    pub const fn policy_stack(&self) -> ContinuityReferenceV1 {
        self.policy_stack
    }

    pub const fn lower_bound(&self) -> u64 {
        self.lower_bound
    }

    pub const fn relation(&self) -> HTimeAcceptanceRelationV1 {
        self.relation
    }

    pub const fn carry_basis(&self) -> HTimeCarryBasisV1 {
        self.carry_basis
    }

    pub(crate) fn from_persisted_parts(
        stable_lineage: ContinuityReferenceV1,
        coordinate: ContinuityReferenceV1,
        policy_stack: ContinuityReferenceV1,
        lower_bound: u64,
        relation: HTimeAcceptanceRelationV1,
        carry_basis: HTimeCarryBasisV1,
    ) -> Self {
        Self {
            stable_lineage,
            coordinate,
            policy_stack,
            lower_bound,
            relation,
            carry_basis,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HTimeAcceptanceErrorV1 {
    #[error("fresh trusted-time bounds are inverted or upper is below the carried base")]
    InvalidFreshBounds,
    #[error("ExactNoLineageChange cannot change lineage, coordinate, or selected Stack")]
    FalseNoLineageChange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carry_plus_fresh_lower_uses_only_max_base_and_lower_and_rejects_invalid_upper() {
        let lineage = reference("lineage");
        let coordinate = reference("coordinate");
        let stack = reference("stack");
        let prior = AcceptedAuthorityTimeFloorV1::context_genesis(
            lineage,
            coordinate,
            stack,
            reference("origin"),
            120,
            130,
        )
        .unwrap();
        let same = AcceptedAuthorityTimeFloorV1::continue_from(
            &prior,
            lineage,
            coordinate,
            stack,
            HTimeCarryBasisV1::ExactNoLineageChange,
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound: 110,
                upper_bound: 999,
            },
        )
        .unwrap();
        assert_eq!(same.lower_bound(), 120);
        assert_eq!(same.relation(), HTimeAcceptanceRelationV1::Same);

        let advanced = AcceptedAuthorityTimeFloorV1::continue_from(
            &prior,
            lineage,
            coordinate,
            stack,
            HTimeCarryBasisV1::ExactNoLineageChange,
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound: 125,
                upper_bound: 140,
            },
        )
        .unwrap();
        assert_eq!(advanced.lower_bound(), 125);
        assert_eq!(advanced.relation(), HTimeAcceptanceRelationV1::Advance);

        for contribution in [
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound: 110,
                upper_bound: 119,
            },
            HTimeContinuationContributionV1::CarryPlusFreshLower {
                lower_bound: 140,
                upper_bound: 130,
            },
        ] {
            assert_eq!(
                AcceptedAuthorityTimeFloorV1::continue_from(
                    &prior,
                    lineage,
                    coordinate,
                    stack,
                    HTimeCarryBasisV1::ExactNoLineageChange,
                    contribution,
                ),
                Err(HTimeAcceptanceErrorV1::InvalidFreshBounds)
            );
        }
    }

    fn reference(seed: &str) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::derive(seed).unwrap()
    }
}
