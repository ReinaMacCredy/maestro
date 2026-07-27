#![cfg(test)]

use crate::domain::vnext::distribution::CommitmentV1;
use crate::domain::vnext::installation::PreStoreCutoverCandidateV1;

use super::InstallationOperationErrorV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedLocatorCommitOutcomeV1 {
    pub observed_old_root_id: CommitmentV1,
    pub committed_candidate_root_id: CommitmentV1,
    pub association_id: CommitmentV1,
}

/// Protected locator implementations must perform one expected-old compare and
/// candidate-root publication under the Stage-4 pre-Store ceremony carrier.
/// A mismatch returns without changing the locator or any managed target.
pub trait ProtectedLocatorCutoverPortV1 {
    fn compare_exchange_candidate(
        &mut self,
        candidate: &PreStoreCutoverCandidateV1,
    ) -> Result<ProtectedLocatorCommitOutcomeV1, InstallationOperationErrorV1>;
}

pub fn commit_prestore_cutover(
    port: &mut impl ProtectedLocatorCutoverPortV1,
    candidate: &PreStoreCutoverCandidateV1,
) -> Result<ProtectedLocatorCommitOutcomeV1, InstallationOperationErrorV1> {
    let outcome = port.compare_exchange_candidate(candidate)?;
    let material = candidate.finality().parts().association.material();
    if outcome.observed_old_root_id != candidate.locator().expected_old_root_id
        || outcome.committed_candidate_root_id != candidate.locator().candidate_store_root_id
        || outcome.association_id.as_bytes() != material.association_id.as_bytes()
    {
        return Err(InstallationOperationErrorV1::InvalidMigrationAssociation);
    }
    Ok(outcome)
}
