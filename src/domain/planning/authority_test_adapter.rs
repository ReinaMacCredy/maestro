//! Test-only parity adapter over the frozen Stage-6 Authority facade.
// TODO(stage8-integration): Remove this non-Scheduling parity adapter on or after 2026-07-24 once every Planning Action uses its owner-specific atomic publication path.

use thiserror::Error;

use crate::domain::authority::{
    ActionRequestIdV1, AdmittedRepositoryActionV1, BootstrapAuthoritySnapshotV1,
    PlanningRepositoryActionAuthorityV1, RepositoryActionAdmissionInputV1,
    RepositoryAuthorityAdmissionErrorV1, RepositoryAuthoritySelectionV1,
    RepositoryDownstreamActionErrorV1, RepositoryDownstreamActionLeafV1,
    RepositoryLeafAuthorityErrorV1, admit_repository_action,
};
use crate::domain::persistence::{StoreError, StoreGenerationV1, StorePublicationViewV1};
use crate::foundation::core::deterministic_cbor;

use super::state::PlanningTransitionDispositionV1;
use super::state::PlanningTransitionV1;

pub(crate) struct AdmittedPlanningTransitionV1 {
    transition: PlanningTransitionV1,
    authority: AdmittedRepositoryActionV1,
}

impl AdmittedPlanningTransitionV1 {
    pub(crate) const fn transition(&self) -> &PlanningTransitionV1 {
        &self.transition
    }

    pub(crate) const fn authority(&self) -> &AdmittedRepositoryActionV1 {
        &self.authority
    }

    pub(crate) fn into_parts(self) -> (PlanningTransitionV1, AdmittedRepositoryActionV1) {
        (self.transition, self.authority)
    }
}

pub(crate) fn admit_planning_transition(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: PlanningTransitionV1,
) -> Result<AdmittedPlanningTransitionV1, PlanningAdmissionErrorV1> {
    if transition.disposition() == PlanningTransitionDispositionV1::Deduplicated {
        return Err(PlanningAdmissionErrorV1::DeduplicatedRequiresNoAuthority);
    }
    validate_actor_provenance(view, generation, selection, &transition)?;
    let action = RepositoryDownstreamActionLeafV1::parse_exact(transition.action_literal())?;
    if action.global_tag() == 105 {
        return Err(PlanningAdmissionErrorV1::SchedulingRequiresAtomicPublication);
    }
    let authority = PlanningRepositoryActionAuthorityV1::new(
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
    Ok(AdmittedPlanningTransitionV1 {
        transition,
        authority: admitted,
    })
}

fn validate_actor_provenance(
    view: &StorePublicationViewV1<'_>,
    generation: &StoreGenerationV1,
    selection: RepositoryAuthoritySelectionV1,
    transition: &PlanningTransitionV1,
) -> Result<(), PlanningAdmissionErrorV1> {
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
        return Err(PlanningAdmissionErrorV1::ActorSnapshotUnavailable);
    };
    if snapshot.actor_binding().principal_id() != transition.actor_principal()
        || snapshot.actor_session().id() != transition.actor_session()
        || snapshot.actor_session().binding_id() != snapshot.actor_binding().id()
    {
        return Err(PlanningAdmissionErrorV1::ActorProvenanceMismatch);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum PlanningAdmissionErrorV1 {
    #[error("deduplicated Planning transition requires no Authority admission")]
    DeduplicatedRequiresNoAuthority,
    #[error("the active generation does not contain one exact selected Authority actor snapshot")]
    ActorSnapshotUnavailable,
    #[error("Planning publisher Principal or Session differs from the selected Authority actor")]
    ActorProvenanceMismatch,
    #[error("Scheduling Policy Binding must use the one-call atomic Authority publication path")]
    SchedulingRequiresAtomicPublication,
    #[error(transparent)]
    DownstreamAction(#[from] RepositoryDownstreamActionErrorV1),
    #[error(transparent)]
    LeafAuthority(#[from] RepositoryLeafAuthorityErrorV1),
    #[error(transparent)]
    AuthorityAdmission(#[from] RepositoryAuthorityAdmissionErrorV1),
    #[error(transparent)]
    Store(#[from] StoreError),
}
