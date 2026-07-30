//! Test-only parity adapter over the frozen Stage-6 Authority facade.
// TODO(stage7-integration): Remove this test-only parity adapter on or after 2026-07-23 once the frozen Stage-6 owner-family constructor is production-callable under strict lint.

use thiserror::Error;

use crate::domain::authority::{
    ActionRequestIdV1, AdmittedRepositoryActionV1, BootstrapAuthoritySnapshotErrorV1,
    BootstrapAuthoritySnapshotV1, CoordinationRepositoryActionAuthorityV1,
    RepositoryActionAdmissionInputV1, RepositoryAuthorityAdmissionErrorV1,
    RepositoryAuthoritySelectionV1, RepositoryDownstreamActionErrorV1,
    RepositoryDownstreamActionLeafV1, RepositoryLeafAuthorityErrorV1, admit_repository_action,
};
use crate::domain::persistence::{StoreError, StoreGenerationV1, StorePublicationViewV1};
use crate::foundation::core::deterministic_cbor::{self, CborError};

use super::state::CoordinationTransitionV1;

pub(crate) struct AdmittedCoordinationTransitionV1 {
    transition: CoordinationTransitionV1,
    authority: AdmittedRepositoryActionV1,
}

impl AdmittedCoordinationTransitionV1 {
    pub(crate) const fn transition(&self) -> &CoordinationTransitionV1 {
        &self.transition
    }

    pub(crate) const fn authority(&self) -> &AdmittedRepositoryActionV1 {
        &self.authority
    }

    pub(crate) fn into_parts(self) -> (CoordinationTransitionV1, AdmittedRepositoryActionV1) {
        (self.transition, self.authority)
    }
}

pub(crate) fn admit_coordination_transition(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: CoordinationTransitionV1,
) -> Result<AdmittedCoordinationTransitionV1, CoordinationAdmissionErrorV1> {
    validate_actor_provenance(view, generation, selection, &transition)?;
    let action = RepositoryDownstreamActionLeafV1::parse_exact(transition.action_literal())?;
    let authority = CoordinationRepositoryActionAuthorityV1::new(
        selection,
        action,
        transition.subject_commitment(),
        transition.owner_basis_commitment(),
        transition.payload_commitment(),
    )?;
    let admitted = admit_repository_action(
        view,
        generation,
        RepositoryActionAdmissionInputV1::new(request_id, authority),
    )?;
    Ok(AdmittedCoordinationTransitionV1 {
        transition,
        authority: admitted,
    })
}

fn validate_actor_provenance(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: &CoordinationTransitionV1,
) -> Result<(), CoordinationAdmissionErrorV1> {
    let mut matches = view
        .active_generation_objects()?
        .into_iter()
        .filter(|object| generation.roots().contains(&object.id()))
        .filter_map(|object| {
            let value_bytes = deterministic_cbor::encode(object.value()).ok()?;
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&value_bytes)
                .ok()
                .filter(|snapshot| {
                    snapshot.snapshot().store_generation == generation.ordinal()
                        && snapshot.actor_binding().id() == selection.actor_binding_id()
                        && snapshot.actor_session().id() == selection.actor_session_id()
                })
        })
        .collect::<Vec<_>>();
    let [snapshot] = matches.as_mut_slice() else {
        return Err(CoordinationAdmissionErrorV1::ActorSnapshotUnavailable);
    };
    if snapshot.actor_binding().principal_id() != transition.actor_principal()
        || snapshot.actor_session().id() != transition.actor_session()
        || snapshot.actor_session().binding_id() != snapshot.actor_binding().id()
    {
        return Err(CoordinationAdmissionErrorV1::ActorProvenanceMismatch);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum CoordinationAdmissionErrorV1 {
    #[error("the active generation does not contain one exact selected Authority actor snapshot")]
    ActorSnapshotUnavailable,
    #[error("Coordination author Principal or Session differs from the selected Authority actor")]
    ActorProvenanceMismatch,
    #[error(transparent)]
    DownstreamAction(#[from] RepositoryDownstreamActionErrorV1),
    #[error(transparent)]
    LeafAuthority(#[from] RepositoryLeafAuthorityErrorV1),
    #[error(transparent)]
    AuthorityAdmission(#[from] RepositoryAuthorityAdmissionErrorV1),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Snapshot(#[from] BootstrapAuthoritySnapshotErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
