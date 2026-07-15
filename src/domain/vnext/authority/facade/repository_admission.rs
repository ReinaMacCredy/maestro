use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::StoreObjectIdV1;
use crate::domain::vnext::persistence::{
    StoreGenerationV1, StoreObjectError, StoreObjectV1, StorePublicationViewV1, StoreRoleV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::publication::AuthoritySchemaV1;
use super::super::{
    ActionAuthorityBasisKindV1, ActionOutcomeV1, ActionRequestIdV1, ActionResultError,
    ActionResultV1, AuthorityContextIdV1, AuthorityContextKindV1, AuthorityContinuityManifestV1,
    AuthorityValidationError, AuthorizationReceiptV1, BootstrapAuthoritySnapshotErrorV1,
    BootstrapAuthoritySnapshotV1, CapacityRootIdV1, CapacityUseDispositionV1, DelegationAncestryV1,
    GovernedCapacityKindV1, GovernedCapacityRootV1, GrantIdV1, OrdinaryBoundedGrantV1,
    OrdinaryGrantDelegationV1, RepositoryGovernedCapacitySlotKindV1, RevocationTargetV1,
    ScopeAtomV1, StateTokenIdV1, SuccessVisibleAuthorityContinuityStateV1, TrustedTimeV1,
    grant_is_revoked_by_closure, validate_delegation, validate_ordinary_authority,
};
use super::repository_leaf_authority::{
    RepositoryLeafAuthorityEvaluationContextV1, RepositoryLeafAuthorityEvaluationErrorV1,
    RepositoryLeafAuthorityInputV1, authenticated_human_carrier_commitment,
    repository_leaf_authority_consumptions,
};

const ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryActionAdmissionInputV1 {
    request_id: ActionRequestIdV1,
    authority: RepositoryLeafAuthorityInputV1,
}

impl RepositoryActionAdmissionInputV1 {
    pub(crate) fn new<A>(request_id: ActionRequestIdV1, authority: A) -> Self
    where
        A: Into<RepositoryLeafAuthorityInputV1>,
    {
        Self {
            request_id,
            authority: authority.into(),
        }
    }
}

pub(crate) struct AdmittedRepositoryActionV1 {
    request_id: ActionRequestIdV1,
    receipt: AuthorizationReceiptV1,
    basis_object: StoreObjectV1,
    current_snapshot_id: StoreObjectIdV1,
    successor_snapshot: StoreObjectV1,
    current_capacity_root_id: StoreObjectIdV1,
    successor_capacity_root: StoreObjectV1,
    capacity_debit: StoreObjectV1,
    leaf_authority_carrier: Option<StoreObjectV1>,
    leaf_authority_consumption: Option<StoreObjectV1>,
    guard_object_id: StoreObjectIdV1,
    state_object_id: StoreObjectIdV1,
    state_token: StateTokenIdV1,
}

impl AdmittedRepositoryActionV1 {
    pub(crate) const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub(crate) fn basis_object(&self) -> &StoreObjectV1 {
        &self.basis_object
    }

    pub(crate) const fn current_snapshot_id(&self) -> StoreObjectIdV1 {
        self.current_snapshot_id
    }

    pub(crate) fn successor_snapshot(&self) -> &StoreObjectV1 {
        &self.successor_snapshot
    }

    pub(crate) const fn current_capacity_root_id(&self) -> StoreObjectIdV1 {
        self.current_capacity_root_id
    }

    pub(crate) fn successor_capacity_root(&self) -> &StoreObjectV1 {
        &self.successor_capacity_root
    }

    pub(crate) fn capacity_debit(&self) -> &StoreObjectV1 {
        &self.capacity_debit
    }

    pub(crate) fn issue_committed_artifacts(
        &self,
        request_object: &StoreObjectV1,
        produced_objects: &[StoreObjectV1],
    ) -> Result<RepositoryAuthorityArtifactsV1, RepositoryAuthorityAdmissionErrorV1> {
        if produced_objects.is_empty() {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidProducedObjects);
        }
        let result = ActionResultV1::new(
            self.request_id,
            ActionOutcomeV1::Committed,
            Some(self.receipt.clone()),
            None,
        )?;
        let leaf_authority_objects = self
            .leaf_authority_carrier
            .iter()
            .chain(self.leaf_authority_consumption.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut receipt_references = vec![
            request_object.id(),
            self.basis_object.id(),
            self.guard_object_id,
            self.state_object_id,
            self.current_snapshot_id,
            self.successor_snapshot.id(),
            self.successor_capacity_root.id(),
            self.capacity_debit.id(),
        ];
        receipt_references.extend(leaf_authority_objects.iter().map(StoreObjectV1::id));
        let receipt_object = authority_object(
            AuthoritySchemaV1::AuthorizationReceipt,
            CborValue::Array(vec![
                bytes(self.receipt.id().as_bytes()),
                bytes(self.receipt.context_id().as_bytes()),
                bytes(self.receipt.request_id().as_bytes()),
                bytes(self.basis_object.id().as_bytes()),
                CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
                CborValue::Bool(true),
                bytes(result.id().as_bytes()),
            ]),
            receipt_references,
        )?;
        let produced_ids = produced_objects
            .iter()
            .map(|object| bytes(object.id().as_bytes()))
            .collect::<Vec<_>>();
        let mut result_references = vec![
            request_object.id(),
            receipt_object.id(),
            self.basis_object.id(),
            self.guard_object_id,
            self.successor_snapshot.id(),
            self.successor_capacity_root.id(),
            self.capacity_debit.id(),
        ];
        result_references.extend(produced_objects.iter().map(StoreObjectV1::id));
        result_references.extend(leaf_authority_objects.iter().map(StoreObjectV1::id));
        let result_object = authority_object(
            AuthoritySchemaV1::ActionResult,
            CborValue::Array(vec![
                bytes(result.id().as_bytes()),
                bytes(result.request_id().as_bytes()),
                CborValue::Unsigned(result.outcome() as u64),
                CborValue::Unsigned(1),
                CborValue::Array(vec![bytes(self.state_token.as_bytes())]),
                CborValue::Array(vec![bytes(self.state_token.as_bytes())]),
                CborValue::Array(vec![bytes(self.receipt.id().as_bytes())]),
                CborValue::Array(produced_ids),
                CborValue::Array(Vec::new()),
                CborValue::optional(None),
                CborValue::optional(None),
            ]),
            result_references,
        )?;
        Ok(RepositoryAuthorityArtifactsV1 {
            logical_result: result,
            receipt_object,
            result_object,
            leaf_authority_objects,
        })
    }
}

pub(crate) struct RepositoryAuthorityArtifactsV1 {
    logical_result: ActionResultV1,
    receipt_object: StoreObjectV1,
    result_object: StoreObjectV1,
    leaf_authority_objects: Vec<StoreObjectV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedRepositoryAuthorityChainV1 {
    terminal_grant_object_id: StoreObjectIdV1,
    terminal_grant: OrdinaryBoundedGrantV1,
    terminal_delegation_object_id: StoreObjectIdV1,
    validated_ordinary_ancestry_object_ids: Vec<StoreObjectIdV1>,
}

struct StoredOrdinaryDelegationV1<'object> {
    object: &'object StoreObjectV1,
    carrier: OrdinaryGrantDelegationV1,
}

impl RepositoryAuthorityArtifactsV1 {
    pub(crate) fn logical_result(&self) -> &ActionResultV1 {
        &self.logical_result
    }

    pub(crate) fn receipt_object(&self) -> &StoreObjectV1 {
        &self.receipt_object
    }

    pub(crate) fn result_object(&self) -> &StoreObjectV1 {
        &self.result_object
    }

    pub(crate) fn leaf_authority_objects(&self) -> &[StoreObjectV1] {
        &self.leaf_authority_objects
    }
}

pub(crate) fn admit_repository_action(
    view: &StorePublicationViewV1<'_>,
    current_generation: &StoreGenerationV1,
    input: RepositoryActionAdmissionInputV1,
) -> Result<AdmittedRepositoryActionV1, RepositoryAuthorityAdmissionErrorV1> {
    if view.role() != StoreRoleV1::Repository
        || current_generation.domain() != view.domain()
        || current_generation.ordinal() == u64::MAX
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let active_objects = view.active_generation_objects()?;
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let mut snapshots = active_objects
        .iter()
        .filter(|object| object.schema_id() == snapshot_schema)
        .filter_map(|object| {
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(object).ok()?)
                .ok()
                .filter(|facts| {
                    facts.context().kind() == AuthorityContextKindV1::RepositoryAuthorityContext
                        && facts.context().store_generation() == current_generation.ordinal()
                        && facts.snapshot().store_generation == current_generation.ordinal()
                })
                .map(|facts| (object, facts))
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let (snapshot_object, facts) = snapshots
        .pop()
        .expect("invariant: exact one-element current Authority snapshot");
    if !current_generation.roots().contains(&snapshot_object.id()) {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }

    let manifest = AuthorityContinuityManifestV1::repository()?;
    let referenced = direct_references(snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    validate_current_guard(
        current_generation,
        &facts,
        &manifest,
        &state_object,
        &guard_object,
    )?;

    let action = input.authority.action();
    let subject_commitment = input.authority.subject_commitment();
    let subject_basis_commitment = input.authority.subject_basis_commitment();
    let selection = input.authority.selection();
    if facts.actor_binding().id() != selection.actor_binding_id()
        || facts.actor_session().id() != selection.actor_session_id()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
    }
    let authority_objects = active_objects
        .iter()
        .filter(|object| {
            current_generation.roots().contains(&object.id())
                || snapshot_object.references().contains(&object.id())
        })
        .collect::<Vec<_>>();
    let resolved = resolve_repository_authority_chain(
        &facts,
        selection.terminal_grant_id(),
        &authority_objects,
    )?;
    let (trusted_time_lower, trusted_time_upper) = match facts.snapshot().trusted_time {
        TrustedTimeV1::Verified {
            lower_bound,
            upper_bound,
        } => (Some(lower_bound), Some(upper_bound)),
        TrustedTimeV1::Unavailable => (None, None),
    };
    let leaf_evaluation_context = RepositoryLeafAuthorityEvaluationContextV1 {
        human_binding_id: facts.responder_binding().id(),
        human_session_id: facts.responder_session().id(),
        human_capable: facts.responder_binding().human_capable()
            && facts.responder_session().binding_id() == facts.responder_binding().id()
            && facts.responder_binding().context_id() == facts.context().context_id()
            && facts.responder_session().context_id() == facts.context().context_id()
            && facts.responder_session().store_generation() == current_generation.ordinal()
            && facts.responder_session().authority_epoch() == facts.snapshot().authority_epoch
            && facts
                .snapshot()
                .trusted_time
                .is_within(facts.responder_binding().validity())?
            && facts
                .snapshot()
                .trusted_time
                .is_within(facts.responder_session().validity())?,
        human_revoked: facts.revocations().revocations().contains(
            RevocationTargetV1::PrincipalBinding(facts.responder_binding().id()),
        ) || facts
            .revocations()
            .revocations()
            .contains(RevocationTargetV1::Session(facts.responder_session().id())),
        authenticated_carrier_commitment: authenticated_human_carrier_commitment(
            facts.responder_session().request_commitment().as_bytes(),
        )?,
        human_valid_until: facts
            .responder_binding()
            .validity()
            .expires_at()
            .min(facts.responder_session().validity().expires_at()),
        trusted_time_lower,
        trusted_time_upper,
        prior_consumptions: repository_leaf_authority_consumptions(&active_objects)?,
    };
    let specialized_authority = input
        .authority
        .evaluate_specialized(&leaf_evaluation_context)?;
    let leaf_authority_carrier = specialized_authority
        .as_ref()
        .map(|authority| authority.carrier_object(vec![snapshot_object.id()]))
        .transpose()?;
    let selected_grant_object_id = resolved.terminal_grant_object_id;
    let selected_grant = &resolved.terminal_grant;
    let delegation_object_id = resolved.terminal_delegation_object_id;
    let validated_ordinary_ancestry_object_ids =
        resolved.validated_ordinary_ancestry_object_ids.clone();
    let capacity_root_id = selected_grant.capacity_root_id();
    let (current_capacity_root_object, current_capacity_root) = current_capacity_root(
        &active_objects,
        current_generation,
        snapshot_object,
        facts.context().context_id(),
        capacity_root_id,
    )?;
    let capacity_transition = current_capacity_root.transition(
        facts.context().context_id(),
        GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
        ),
        current_capacity_root.spent(),
        CapacityUseDispositionV1::FreshCommit,
    )?;
    let successor_capacity_root = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        capacity_transition.root().schema_value()?,
        vec![current_capacity_root_object.id()],
    )?;
    let capacity_debit = authority_object(
        AuthoritySchemaV1::GovernedCapacityDebit,
        capacity_transition
            .debit()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable)?
            .schema_value()?,
        vec![
            current_capacity_root_object.id(),
            successor_capacity_root.id(),
        ],
    )?;
    let required_scope = ScopeAtomV1::new(
        action.literal(),
        &render_digest(subject_commitment),
        facts.snapshot().subject_revision,
    )?;
    validate_ordinary_authority(
        facts.snapshot(),
        facts.actor_binding(),
        facts.actor_session(),
        selected_grant.grant(),
        &required_scope,
        facts.revocations().revocations(),
    )?;
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }

    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let basis_commitment = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-action-authority-basis.v1")?,
        bytes(input.request_id.as_bytes()),
        CborValue::text(action.literal())?,
        CborValue::Unsigned(action.global_tag()),
        CborValue::Unsigned(action.owner_tag()),
        CborValue::Unsigned(action.local_tag()),
        CborValue::text(action.owner_descriptor_id())?,
        CborValue::text(action.descriptor_id())?,
        CborValue::Unsigned(action.protocol_revision()),
        CborValue::text(action.manifest_id())?,
        CborValue::text(action.grammar_id())?,
        bytes(&subject_commitment),
        bytes(&subject_basis_commitment),
        bytes(current_generation.id().as_bytes()),
        bytes(current_generation.contract_root_id().as_bytes()),
        bytes(facts.context().context_id().as_bytes()),
        CborValue::Unsigned(facts.snapshot().authority_epoch),
        bytes(selection.actor_binding_id().as_bytes()),
        bytes(selection.actor_session_id().as_bytes()),
        bytes(selection.terminal_grant_id().as_bytes()),
        CborValue::optional(
            specialized_authority
                .as_ref()
                .map(|authority| bytes(&authority.leaf_commitment())),
        ),
        bytes(&guard_digest),
        bytes(facts.continuity().state_token().as_bytes()),
    ]))?;
    let mut basis_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        selected_grant_object_id,
        current_capacity_root_object.id(),
    ];
    basis_references.extend(leaf_authority_carrier.iter().map(StoreObjectV1::id));
    let basis_object = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime as u64),
            bytes(facts.context().context_id().as_bytes()),
            bytes(&basis_commitment),
        ]),
        basis_references,
    )?;
    let leaf_authority_consumption = specialized_authority
        .as_ref()
        .zip(leaf_authority_carrier.as_ref())
        .map(|(authority, carrier)| {
            authority.consumption_object(
                input.request_id,
                current_generation.id(),
                basis_object.id(),
                carrier.id(),
            )
        })
        .transpose()?;
    let receipt = AuthorizationReceiptV1::new(
        input.request_id,
        facts.context().context_id(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        facts.continuity().state_token(),
        facts.continuity().state_token(),
    )?;
    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::Unavailable)?;
    let successor_facts = facts.continue_at_store_generation(
        next_generation,
        facts.continuity().manifest_id(),
        facts.continuity().guard_kind(),
        facts.continuity().state_token(),
    )?;
    let mut successor_snapshot_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        selected_grant_object_id,
        delegation_object_id,
        successor_capacity_root.id(),
        capacity_debit.id(),
    ];
    successor_snapshot_references.extend(validated_ordinary_ancestry_object_ids);
    successor_snapshot_references.extend(leaf_authority_carrier.iter().map(StoreObjectV1::id));
    successor_snapshot_references.extend(leaf_authority_consumption.iter().map(StoreObjectV1::id));
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_snapshot_references,
    )?;
    Ok(AdmittedRepositoryActionV1 {
        request_id: input.request_id,
        receipt,
        basis_object,
        current_snapshot_id: snapshot_object.id(),
        successor_snapshot,
        current_capacity_root_id: current_capacity_root_object.id(),
        successor_capacity_root,
        capacity_debit,
        leaf_authority_carrier,
        leaf_authority_consumption,
        guard_object_id: guard_object.id(),
        state_object_id: state_object.id(),
        state_token: facts.continuity().state_token(),
    })
}

fn resolve_repository_authority_chain(
    facts: &BootstrapAuthoritySnapshotV1,
    terminal_grant_id: GrantIdV1,
    authority_objects: &[&StoreObjectV1],
) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
    let grant_schema = OrdinaryBoundedGrantV1::store_schema_id()?;
    let grants = authority_objects
        .iter()
        .filter(|object| object.schema_id() == grant_schema)
        .map(|object| {
            Ok((
                *object,
                OrdinaryBoundedGrantV1::from_canonical_bytes(&deterministic_cbor::encode(
                    object.value(),
                )?)?,
            ))
        })
        .collect::<Result<Vec<_>, RepositoryAuthorityAdmissionErrorV1>>()?;
    let mut terminal_grants = grants
        .iter()
        .filter(|(_, grant)| grant.grant().id() == terminal_grant_id)
        .collect::<Vec<_>>();
    if terminal_grants.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
    }
    let (terminal_grant_object, terminal_grant) = terminal_grants
        .pop()
        .expect("invariant: exact one-element terminal ordinary Grant");
    let expected_context_id = facts.context().context_id();
    let expected_capacity_root_id = terminal_grant.capacity_root_id();

    let delegation_schema = OrdinaryGrantDelegationV1::store_schema_id()?;
    let mut delegations = Vec::new();
    for object in authority_objects
        .iter()
        .filter(|object| object.schema_id() == delegation_schema)
    {
        let encoded = deterministic_cbor::encode(object.value())?;
        let mut carriers = grants
            .iter()
            .filter_map(|(_, child)| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(&encoded, child).ok()
            })
            .collect::<Vec<_>>();
        if carriers.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        delegations.push(StoredOrdinaryDelegationV1 {
            object,
            carrier: carriers
                .pop()
                .expect("invariant: exact one-element canonical Delegation carrier"),
        });
    }

    let mut visited = BTreeSet::new();
    let mut reverse_chain = Vec::new();
    let mut child_object = *terminal_grant_object;
    let mut child = terminal_grant;
    let root_path = loop {
        if !visited.insert(child.grant().id())
            || child.grant().context_id() != expected_context_id
            || child.capacity_root_id() != expected_capacity_root_id
            || !facts
                .snapshot()
                .trusted_time
                .is_within(child.grant().validity())?
        {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        let mut child_delegations = delegations
            .iter()
            .filter(|entry| {
                entry.carrier.delegation().child_grant_id == child.grant().id()
                    && entry.carrier.context_id() == expected_context_id
                    && entry.carrier.capacity_root_id() == expected_capacity_root_id
            })
            .collect::<Vec<_>>();
        if child_delegations.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        let delegation = child_delegations
            .pop()
            .expect("invariant: exact one-element Delegation for ancestry hop");
        reverse_chain.push((child_object, child, delegation));

        let parent_id = child
            .grant()
            .parent_grant_id()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
        let ordinary_parents = grants
            .iter()
            .filter(|(_, grant)| grant.grant().id() == parent_id)
            .collect::<Vec<_>>();
        let root_paths = facts
            .g0_candidate_paths()
            .iter()
            .filter(|path| path.grant().id() == parent_id)
            .collect::<Vec<_>>();
        if ordinary_parents.len() + root_paths.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        if let [(parent_object, parent)] = ordinary_parents.as_slice() {
            child_object = *parent_object;
            child = parent;
            continue;
        }
        break root_paths[0];
    };

    if !root_path.complete()
        || root_path.store_generation() != facts.context().store_generation()
        || root_path.store_generation() != facts.snapshot().store_generation
        || root_path.authority_epoch() != facts.snapshot().authority_epoch
        || root_path.trust_root_revision() != facts.snapshot().trust_root_revision
        || root_path.grant().context_id() != expected_context_id
        || root_path
            .root_contributions()
            .iter()
            .any(|root_id| *root_id != expected_capacity_root_id)
        || !facts
            .snapshot()
            .trusted_time
            .is_within(root_path.grant().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }

    let mut ancestry_grant_ids = vec![root_path.grant().id()];
    let mut ancestry_principal_ids = vec![root_path.grant().grantee_principal_id()];
    let mut has_bounded_root = root_path
        .root_contributions()
        .contains(&expected_capacity_root_id);
    let mut structural_parent = root_path.grant().definition();
    structural_parent.delegation_depth_remaining = u8::MAX;
    let mut parent = structural_parent.validate()?;
    for (_, child, delegation) in reverse_chain.iter().rev() {
        let ancestry = DelegationAncestryV1::new(
            ancestry_grant_ids.clone(),
            ancestry_principal_ids.clone(),
            has_bounded_root,
        )?;
        validate_delegation(
            &parent,
            child.grant(),
            &delegation.carrier.delegation(),
            &ancestry,
        )?;
        ancestry_grant_ids.push(child.grant().id());
        ancestry_principal_ids.push(child.grant().grantee_principal_id());
        has_bounded_root = true;
        parent = child.grant().clone();
    }

    let chain_grants = reverse_chain
        .iter()
        .map(|(_, grant, _)| (*grant).clone())
        .collect::<Vec<_>>();
    if grant_is_revoked_by_closure(
        terminal_grant,
        &chain_grants,
        facts.revocations().revocations(),
    )? {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let terminal_delegation = reverse_chain
        .first()
        .expect("invariant: ordinary Grant ancestry contains its terminal hop")
        .2;
    let validated_ordinary_ancestry_object_ids = reverse_chain
        .iter()
        .flat_map(|(grant_object, _, delegation)| [grant_object.id(), delegation.object.id()])
        .collect();
    Ok(ResolvedRepositoryAuthorityChainV1 {
        terminal_grant_object_id: terminal_grant_object.id(),
        terminal_grant: terminal_grant.clone(),
        terminal_delegation_object_id: terminal_delegation.object.id(),
        validated_ordinary_ancestry_object_ids,
    })
}

pub(crate) fn admit_repository_authority_candidate(
    facts: &BootstrapAuthoritySnapshotV1,
    expected_capacity_root_id: CapacityRootIdV1,
    candidate: &OrdinaryBoundedGrantV1,
    delegation: &OrdinaryGrantDelegationV1,
) -> Result<(), RepositoryAuthorityAdmissionErrorV1> {
    if candidate.capacity_root_id() != expected_capacity_root_id
        || delegation.capacity_root_id() != expected_capacity_root_id
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let parent_id = candidate
        .grant()
        .parent_grant_id()
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let mut parents = facts
        .g0_candidate_paths()
        .iter()
        .filter(|path| {
            path.grant().id() == parent_id
                && path.grant().context_id() == candidate.grant().context_id()
                && path.store_generation() == facts.context().store_generation()
                && path.complete()
        })
        .collect::<Vec<_>>();
    if parents.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let parent = parents
        .pop()
        .expect("invariant: exact one-element root-attached Grant parent");
    let ancestry = DelegationAncestryV1::new(
        vec![parent.grant().id()],
        vec![parent.grant().grantee_principal_id()],
        false,
    )?;
    let mut structural_parent = parent.grant().definition();
    structural_parent.delegation_depth_remaining = u8::MAX;
    validate_delegation(
        &structural_parent.validate()?,
        candidate.grant(),
        &delegation.delegation(),
        &ancestry,
    )?;
    Ok(())
}

fn validate_current_guard(
    current_generation: &StoreGenerationV1,
    facts: &BootstrapAuthoritySnapshotV1,
    manifest: &AuthorityContinuityManifestV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
) -> Result<(), RepositoryAuthorityAdmissionErrorV1> {
    let state = SuccessVisibleAuthorityContinuityStateV1::decode(
        &object_value_bytes(state_object)?,
        manifest,
    )?;
    let CborValue::Array(guard_fields) = guard_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    let CborValue::Array(state_fields) = state_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    if guard_fields.len() != 26
        || state_fields.len() != 26
        || !matches!(&guard_fields[0], CborValue::Text(domain) if domain == "maestro.vnext.authority-transition-guard-evaluation.v1")
        || guard_fields[3]
            != CborValue::Unsigned(AuthorityContextKindV1::RepositoryAuthorityContext as u64)
        || exact_digest(&guard_fields[4])? != *facts.context().context_id().as_bytes()
        || !matches!(guard_fields[5], CborValue::Unsigned(value) if value > 0 && value <= current_generation.ordinal())
        || guard_fields[6] != CborValue::Unsigned(facts.snapshot().authority_epoch)
        || exact_digest(&guard_fields[7])? != *facts.continuity().manifest_id().as_bytes()
        || exact_digest(&guard_fields[8])? != *state.closure_id().as_bytes()
        || exact_digest(&state_fields[25])? != guard_digest
        || state.context_kind() != AuthorityContextKindV1::RepositoryAuthorityContext
        || state.context_id() != facts.context().context_id()
        || state.store_generation() > current_generation.ordinal()
        || state.authority_epoch() != facts.snapshot().authority_epoch
        || state.manifest_id() != facts.continuity().manifest_id()
        || state.state_token() != facts.continuity().state_token()
        || state.guard_kind() != facts.continuity().guard_kind()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    }
    Ok(())
}

fn current_capacity_root(
    active_objects: &[StoreObjectV1],
    current_generation: &StoreGenerationV1,
    snapshot_object: &StoreObjectV1,
    context_id: AuthorityContextIdV1,
    expected_id: CapacityRootIdV1,
) -> Result<(StoreObjectV1, GovernedCapacityRootV1), RepositoryAuthorityAdmissionErrorV1> {
    let schema_id = AuthoritySchemaV1::GovernedCapacityRoot.id()?;
    let mut roots = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .filter(|object| {
            current_generation.roots().contains(&object.id())
                || snapshot_object.references().contains(&object.id())
        })
        .filter_map(|object| {
            parse_capacity_root(object)
                .ok()
                .filter(|root| root.id() == expected_id)
                .map(|root| (object.clone(), root))
        })
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    let (object, root) = roots
        .pop()
        .expect("invariant: exact one-element governed-capacity root");
    if root.context_kind() != AuthorityContextKindV1::RepositoryAuthorityContext
        || root.context_id() != context_id
        || root.kind()
            != GovernedCapacityKindV1::Repository(
                RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
            )
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    Ok((object, root))
}

fn parse_capacity_root(
    object: &StoreObjectV1,
) -> Result<GovernedCapacityRootV1, RepositoryAuthorityAdmissionErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if fields.len() != 7
        || !matches!(&fields[0], CborValue::Text(domain) if domain == GovernedCapacityRootV1::SCHEMA_DOMAIN)
        || fields[2]
            != CborValue::Unsigned(AuthorityContextKindV1::RepositoryAuthorityContext as u64)
        || fields[4]
            != CborValue::Unsigned(
                RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation as u64,
            )
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let initial_max = u32::try_from(exact_unsigned(&fields[5])?)
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let spent = u32::try_from(exact_unsigned(&fields[6])?)
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    Ok(GovernedCapacityRootV1::from_persisted_state(
        CapacityRootIdV1::from_digest(exact_digest(&fields[1])?),
        AuthorityContextKindV1::RepositoryAuthorityContext,
        AuthorityContextIdV1::from_digest(exact_digest(&fields[3])?),
        GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
        ),
        initial_max,
        spent,
    )?)
}

fn exact_unsigned(value: &CborValue) -> Result<u64, RepositoryAuthorityAdmissionErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier),
    }
}

fn direct_references(
    object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
) -> Result<Vec<StoreObjectV1>, RepositoryAuthorityAdmissionErrorV1> {
    object
        .references()
        .iter()
        .map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
                .cloned()
                .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority)
        })
        .collect()
}

fn one_schema_object(
    objects: &[StoreObjectV1],
    schema: AuthoritySchemaV1,
) -> Result<StoreObjectV1, RepositoryAuthorityAdmissionErrorV1> {
    let schema_id = schema.id()?;
    let mut matches = objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .cloned()
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    Ok(matches
        .pop()
        .expect("invariant: exact one-element Authority schema match"))
}

fn authority_object(
    schema: AuthoritySchemaV1,
    value: CborValue,
    mut references: Vec<StoreObjectIdV1>,
) -> Result<StoreObjectV1, RepositoryAuthorityAdmissionErrorV1> {
    if !schema.accepts_value(&value) {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(schema.id()?, value, references)?)
}

fn object_value_bytes(
    object: &StoreObjectV1,
) -> Result<Vec<u8>, RepositoryAuthorityAdmissionErrorV1> {
    Ok(deterministic_cbor::encode(object.value())?)
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], RepositoryAuthorityAdmissionErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard)
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn render_digest(value: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

#[derive(Debug, Error)]
pub(crate) enum RepositoryAuthorityAdmissionErrorV1 {
    #[error("Repository action Authority is unavailable")]
    Unavailable,
    #[error("the current Repository Authority snapshot is absent, ambiguous, or stale")]
    InvalidCurrentAuthority,
    #[error("the current Repository Authority transition guard is substituted or stale")]
    InvalidCurrentGuard,
    #[error("the selected Binding, Session, or Grant does not match current Authority facts")]
    AuthoritySelectionMismatch,
    #[error("the selected Repository action capacity basis is unavailable")]
    CapacityUnavailable,
    #[error("the Authority carrier has an invalid canonical schema shape")]
    InvalidAuthorityCarrier,
    #[error("a committed Repository action must produce at least one owner object")]
    InvalidProducedObjects,
    #[error(transparent)]
    Store(#[from] crate::domain::vnext::persistence::StoreError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
    #[error(transparent)]
    AuthorityValidation(#[from] AuthorityValidationError),
    #[error(transparent)]
    Capacity(#[from] super::super::CapacityError),
    #[error(transparent)]
    AuthoritySnapshot(#[from] BootstrapAuthoritySnapshotErrorV1),
    #[error(transparent)]
    AuthorityState(#[from] super::super::AuthorityContinuityStateError),
    #[error(transparent)]
    AuthorityContinuity(#[from] super::super::AuthorityContinuityError),
    #[error(transparent)]
    ActionResult(#[from] ActionResultError),
    #[error(transparent)]
    LeafAuthority(#[from] RepositoryLeafAuthorityEvaluationErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::super::super::continuity::StoreAllocatedContinuityStateTokenV1;
    use super::super::super::*;
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum AuthorityFixtureModeV1 {
        Valid,
        MultiHop,
        MultiHopCycle,
        MultiHopExpiredAncestor,
        MultiHopForeignRoot,
        MultiHopNonAttenuated,
        MultiHopRevokedAncestor,
        MultiHopStaleRoot,
        OrphanedGrant,
        RevokedGrant,
        ExpiredGrant,
        SubstitutedGuard,
    }

    pub(crate) struct RepositoryAuthorityFixtureV1 {
        pub(crate) objects: Vec<StoreObjectV1>,
        pub(crate) authority_root_id: StoreObjectIdV1,
        pub(crate) selection: RepositoryAuthoritySelectionV1,
        pub(crate) authenticated_human: RepositoryAuthenticatedHumanV1,
        pub(crate) leaf_authority_expires_at: u64,
    }

    pub(crate) fn repository_authority_fixture(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
    ) -> RepositoryAuthorityFixtureV1 {
        let multi_hop = matches!(
            mode,
            AuthorityFixtureModeV1::MultiHop
                | AuthorityFixtureModeV1::MultiHopCycle
                | AuthorityFixtureModeV1::MultiHopExpiredAncestor
                | AuthorityFixtureModeV1::MultiHopForeignRoot
                | AuthorityFixtureModeV1::MultiHopNonAttenuated
                | AuthorityFixtureModeV1::MultiHopRevokedAncestor
                | AuthorityFixtureModeV1::MultiHopStaleRoot
        );
        let manifest = AuthorityContinuityManifestV1::repository().unwrap();
        let context_id = AuthorityContextIdV1::derive("stage3-repository-context").unwrap();
        let (closure, guard, state) = continuity_generation(&manifest, context_id);
        let manifest_object = authority_object(
            AuthoritySchemaV1::AuthorityContinuityManifest,
            manifest.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let closure_object = authority_object(
            AuthoritySchemaV1::AuthorityContinuityClosure,
            closure.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let guard_object = authority_object(
            AuthoritySchemaV1::AdmittedTransitionGuard,
            guard.schema_value().unwrap(),
            vec![closure_object.id()],
        )
        .unwrap();
        let state_object = authority_object(
            AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
            state.schema_value().unwrap(),
            vec![closure_object.id(), guard_object.id()],
        )
        .unwrap();
        let selected_guard = if mode == AuthorityFixtureModeV1::SubstitutedGuard {
            let CborValue::Array(mut fields) = guard.schema_value().unwrap() else {
                unreachable!("guard schema is an array")
            };
            fields[4] = bytes(&[93; 32]);
            authority_object(
                AuthoritySchemaV1::AdmittedTransitionGuard,
                CborValue::Array(fields),
                vec![closure_object.id()],
            )
            .unwrap()
        } else {
            guard_object.clone()
        };

        let validity = HalfOpenValidityV1::new(100, 200).unwrap();
        let context =
            AuthorityContextV1::repository(context_id, "stage3-repository-installation", 1, 7, 11)
                .unwrap();
        let actor_principal = PrincipalIdV1::derive("stage3-actor-principal").unwrap();
        let actor_binding = PrincipalBindingV1::new(
            PrincipalBindingIdV1::derive("stage3-actor-binding").unwrap(),
            actor_principal,
            context_id,
            11,
            1,
            validity,
            false,
        )
        .unwrap();
        let responder_binding = PrincipalBindingV1::new(
            PrincipalBindingIdV1::derive("stage3-responder-binding").unwrap(),
            PrincipalIdV1::derive("stage3-responder-principal").unwrap(),
            context_id,
            11,
            1,
            validity,
            true,
        )
        .unwrap();
        let target_head = StateTokenIdV1::derive("stage3-target-head").unwrap();
        let target = TargetActionProjectionV1::new(
            BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
            "stage3-recovery-selection",
            1,
            TargetActionOwnerV1::Authority,
            TargetActionProtocolV1::RecoveryCommitmentSelection,
            TargetActionEffectKindV1::Rotate,
            "sha256:stage3-effect-closure",
            TargetExpectedHeadsV1::new(context_id, 1, 7, 11, 1, target_head).unwrap(),
            validity,
        )
        .unwrap();
        let target_commitment = target.target_action_commitment().unwrap();
        let request_commitment = target_commitment.render();
        let actor_session = SessionV1::new(
            SessionIdV1::derive("stage3-actor-session").unwrap(),
            actor_binding.id(),
            context_id,
            1,
            7,
            &request_commitment,
            validity,
        )
        .unwrap();
        let responder_session = SessionV1::new(
            SessionIdV1::derive("stage3-responder-session").unwrap(),
            responder_binding.id(),
            context_id,
            1,
            7,
            &request_commitment,
            validity,
        )
        .unwrap();
        let consent = ConsentSlotEvaluationFactsV1::derive_for_target(&target, validity).unwrap();
        let procedure = StateTokenIdV1::derive("stage3-interaction-procedure").unwrap();
        let subject = BootstrapInteractionSubjectV1::new(
            context_id,
            StateTokenIdV1::derive("stage3-interaction-plan").unwrap(),
            ActionRequestIdV1::derive("stage3-interaction-attempt").unwrap(),
            responder_binding.id(),
            1,
            target_commitment,
            consent.binding().clone(),
            StateTokenIdV1::derive("stage3-option-map").unwrap(),
            StateTokenIdV1::derive("stage3-affirmative-option").unwrap(),
        );
        let presentation = BootstrapMandatePresentationObservationV1::new(
            subject.clone(),
            StateTokenIdV1::derive("stage3-interaction-carrier").unwrap(),
            procedure,
        )
        .unwrap();
        let response = BootstrapMandateResponseObservationV1::new(
            subject,
            presentation.id(),
            BootstrapResponseDispositionV1::Affirmative,
            StateTokenIdV1::derive("stage3-affirmative-option").unwrap(),
        )
        .unwrap();
        let interaction = BootstrapMandateInteractionObservationJoinV1::new(
            &presentation,
            &response,
            responder_session.id(),
            procedure,
        )
        .unwrap();

        let ordinary_scope = scopes
            .into_iter()
            .map(|(action, subject)| ScopeAtomV1::new(action, &render_digest(subject), 1).unwrap())
            .collect::<Vec<_>>();
        let bootstrap_scope =
            ScopeAtomV1::new("IssueBootstrapMandate", &request_commitment, 1).unwrap();
        let bootstrap_grant = GrantDefinitionV1 {
            id: GrantIdV1::derive("stage3-bootstrap-only-grant").unwrap(),
            context_id,
            grantee_principal_id: PrincipalIdV1::derive("stage3-g0-principal").unwrap(),
            parent_grant_id: None,
            delegation_id: None,
            terminal_scope: GrantScopeV1::new(vec![bootstrap_scope]).unwrap(),
            delegable_scope: GrantScopeV1::new(ordinary_scope.clone()).unwrap(),
            validity,
            delegation_depth_remaining: 1,
            authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
        }
        .validate()
        .unwrap();
        let bootstrap_grant_id = bootstrap_grant.id();
        let bootstrap_path = BootstrapG0PathV1::new(
            GenesisGrantIdV1::derive(&bootstrap_grant.id().render()).unwrap(),
            bootstrap_grant,
            if mode == AuthorityFixtureModeV1::MultiHopStaleRoot {
                0
            } else {
                1
            },
            7,
            11,
            true,
            vec![],
        )
        .unwrap();

        let capacity_root_id = CapacityRootIdV1::derive("stage3-ordinary-capacity").unwrap();
        let ordinary_parent_grant_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let ordinary_parent_delegation_id =
            DelegationIdV1::derive("stage3-ordinary-parent-delegation").unwrap();
        let ordinary_grant_id = GrantIdV1::derive("stage3-ordinary-grant").unwrap();
        let ordinary_delegation_id = DelegationIdV1::derive("stage3-ordinary-delegation").unwrap();
        let ordinary_parent_capacity_root_id =
            if mode == AuthorityFixtureModeV1::MultiHopForeignRoot {
                CapacityRootIdV1::derive("stage3-foreign-ordinary-capacity").unwrap()
            } else {
                capacity_root_id
            };
        let ordinary_parent = multi_hop.then(|| {
            OrdinaryBoundedGrantV1::new(
                GrantDefinitionV1 {
                    id: ordinary_parent_grant_id,
                    context_id,
                    grantee_principal_id: PrincipalIdV1::derive("stage3-intermediate-principal")
                        .unwrap(),
                    parent_grant_id: Some(if mode == AuthorityFixtureModeV1::MultiHopCycle {
                        ordinary_grant_id
                    } else {
                        bootstrap_grant_id
                    }),
                    delegation_id: Some(ordinary_parent_delegation_id),
                    terminal_scope: GrantScopeV1::new(vec![]).unwrap(),
                    delegable_scope: GrantScopeV1::new(ordinary_scope.clone()).unwrap(),
                    validity: if mode == AuthorityFixtureModeV1::MultiHopExpiredAncestor {
                        HalfOpenValidityV1::new(10, 20).unwrap()
                    } else {
                        validity
                    },
                    delegation_depth_remaining: if mode
                        == AuthorityFixtureModeV1::MultiHopNonAttenuated
                    {
                        0
                    } else {
                        1
                    },
                    authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(
                        ordinary_parent_capacity_root_id,
                    ),
                }
                .validate()
                .unwrap(),
            )
            .unwrap()
        });
        let ordinary_parent_delegation = ordinary_parent.as_ref().map(|parent| {
            OrdinaryGrantDelegationV1::new(
                context_id,
                ordinary_parent_capacity_root_id,
                DelegationV1::new(
                    ordinary_parent_delegation_id,
                    if mode == AuthorityFixtureModeV1::MultiHopCycle {
                        ordinary_grant_id
                    } else {
                        bootstrap_grant_id
                    },
                    ordinary_parent_grant_id,
                ),
                parent,
            )
            .unwrap()
        });
        let ordinary_validity = if mode == AuthorityFixtureModeV1::ExpiredGrant {
            HalfOpenValidityV1::new(10, 20).unwrap()
        } else {
            validity
        };
        let ordinary_grant = GrantDefinitionV1 {
            id: ordinary_grant_id,
            context_id,
            grantee_principal_id: actor_principal,
            parent_grant_id: Some(if mode == AuthorityFixtureModeV1::OrphanedGrant {
                GrantIdV1::derive("stage3-orphaned-parent-grant").unwrap()
            } else {
                ordinary_parent
                    .as_ref()
                    .map_or(bootstrap_grant_id, |parent| parent.grant().id())
            }),
            delegation_id: Some(ordinary_delegation_id),
            terminal_scope: GrantScopeV1::new(ordinary_scope).unwrap(),
            delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
            validity: ordinary_validity,
            delegation_depth_remaining: 0,
            authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(capacity_root_id),
        }
        .validate()
        .unwrap();
        let ordinary_grant = OrdinaryBoundedGrantV1::new(ordinary_grant).unwrap();
        let ordinary_delegation = OrdinaryGrantDelegationV1::new(
            context_id,
            capacity_root_id,
            DelegationV1::new(
                ordinary_delegation_id,
                if mode == AuthorityFixtureModeV1::OrphanedGrant {
                    GrantIdV1::derive("stage3-orphaned-parent-grant").unwrap()
                } else {
                    ordinary_parent
                        .as_ref()
                        .map_or(bootstrap_grant_id, |parent| parent.grant().id())
                },
                ordinary_grant_id,
            ),
            &ordinary_grant,
        )
        .unwrap();
        let revocations = match mode {
            AuthorityFixtureModeV1::RevokedGrant => {
                RevocationSetV1::new(vec![RevocationTargetV1::Grant(ordinary_grant_id)]).unwrap()
            }
            AuthorityFixtureModeV1::MultiHopRevokedAncestor => {
                RevocationSetV1::new(vec![RevocationTargetV1::Grant(ordinary_parent_grant_id)])
                    .unwrap()
            }
            _ => RevocationSetV1::empty(),
        };
        let revocations = AuthorityRevocationSetV1::new(context_id, revocations);
        let facts = BootstrapAuthoritySnapshotV1::new(
            context,
            AuthoritySnapshotV1::new(
                context_id,
                1,
                7,
                11,
                1,
                TrustedTimeV1::verified(120, 130).unwrap(),
            ),
            actor_binding,
            actor_session,
            responder_binding,
            responder_session,
            vec![bootstrap_path],
            revocations,
            Some(interaction),
            procedure,
            target,
            target_head,
            consent,
            BootstrapContinuityTransitionProofV1::new(
                context_id,
                1,
                7,
                11,
                manifest.id(),
                state.guard_kind(),
                state.state_token(),
                validity,
            ),
        )
        .unwrap();

        let context_object = authority_object(
            AuthoritySchemaV1::AuthorityContext,
            facts.context().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let actor_binding_object = authority_object(
            AuthoritySchemaV1::PrincipalBinding,
            facts.actor_binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let responder_binding_object = authority_object(
            AuthoritySchemaV1::PrincipalBinding,
            facts.responder_binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let actor_session_object = authority_object(
            AuthoritySchemaV1::Session,
            facts.actor_session().schema_value().unwrap(),
            vec![actor_binding_object.id()],
        )
        .unwrap();
        let responder_session_object = authority_object(
            AuthoritySchemaV1::Session,
            facts.responder_session().schema_value().unwrap(),
            vec![responder_binding_object.id()],
        )
        .unwrap();
        let bootstrap_grant_object = authority_object(
            AuthoritySchemaV1::BootstrapGenesisGrant,
            facts.g0_candidate_paths()[0]
                .genesis_grant()
                .schema_value()
                .unwrap(),
            vec![],
        )
        .unwrap();
        let ordinary_parent_grant_object = ordinary_parent.as_ref().map(|parent| {
            authority_object(
                AuthoritySchemaV1::OrdinaryBoundedGrant,
                parent.schema_value().unwrap(),
                vec![bootstrap_grant_object.id()],
            )
            .unwrap()
        });
        let ordinary_parent_delegation_object = ordinary_parent_delegation.as_ref().map(|entry| {
            authority_object(
                AuthoritySchemaV1::OrdinaryGrantDelegation,
                entry.schema_value().unwrap(),
                vec![
                    ordinary_parent_grant_object.as_ref().unwrap().id(),
                    bootstrap_grant_object.id(),
                ],
            )
            .unwrap()
        });
        let revocations_object = authority_object(
            AuthoritySchemaV1::RevocationSet,
            facts.revocations().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let interaction_object = authority_object(
            AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin,
            facts.interaction_join().unwrap().schema_value().unwrap(),
            vec![responder_session_object.id()],
        )
        .unwrap();
        let consent_object = authority_object(
            AuthoritySchemaV1::ConsentSlotBindingParameter,
            facts.consent_slot().binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let ordinary_grant_object = authority_object(
            AuthoritySchemaV1::OrdinaryBoundedGrant,
            ordinary_grant.schema_value().unwrap(),
            vec![
                ordinary_parent_grant_object
                    .as_ref()
                    .map_or(bootstrap_grant_object.id(), StoreObjectV1::id),
            ],
        )
        .unwrap();
        let ordinary_delegation_object = authority_object(
            AuthoritySchemaV1::OrdinaryGrantDelegation,
            ordinary_delegation.schema_value().unwrap(),
            vec![
                ordinary_grant_object.id(),
                ordinary_parent_grant_object
                    .as_ref()
                    .map_or(bootstrap_grant_object.id(), StoreObjectV1::id),
            ],
        )
        .unwrap();
        let capacity_root = GovernedCapacityRootV1::new(
            capacity_root_id,
            AuthorityContextKindV1::RepositoryAuthorityContext,
            context_id,
            GovernedCapacityKindV1::Repository(
                RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
            ),
            32,
        )
        .unwrap();
        let capacity_root_object = authority_object(
            AuthoritySchemaV1::GovernedCapacityRoot,
            capacity_root.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let mut authority_root_references = vec![
            manifest_object.id(),
            closure_object.id(),
            selected_guard.id(),
            state_object.id(),
            context_object.id(),
            actor_binding_object.id(),
            responder_binding_object.id(),
            actor_session_object.id(),
            responder_session_object.id(),
            bootstrap_grant_object.id(),
            revocations_object.id(),
            interaction_object.id(),
            consent_object.id(),
            ordinary_grant_object.id(),
            ordinary_delegation_object.id(),
            capacity_root_object.id(),
        ];
        authority_root_references.extend(
            ordinary_parent_grant_object
                .iter()
                .chain(ordinary_parent_delegation_object.iter())
                .map(StoreObjectV1::id),
        );
        let authority_root = authority_object(
            AuthoritySchemaV1::BootstrapAuthoritySnapshot,
            facts.schema_value().unwrap(),
            authority_root_references,
        )
        .unwrap();
        let mut objects = vec![
            manifest_object,
            closure_object,
            guard_object,
            selected_guard,
            state_object,
            context_object,
            actor_binding_object,
            responder_binding_object,
            actor_session_object,
            responder_session_object,
            bootstrap_grant_object,
            revocations_object,
            interaction_object,
            consent_object,
            authority_root.clone(),
            ordinary_grant_object.clone(),
            ordinary_delegation_object.clone(),
            capacity_root_object.clone(),
        ];
        objects.extend(ordinary_parent_grant_object);
        objects.extend(ordinary_parent_delegation_object);
        objects.sort_by_key(StoreObjectV1::id);
        objects.dedup_by_key(|object| object.id());
        RepositoryAuthorityFixtureV1 {
            objects,
            authority_root_id: authority_root.id(),
            selection: RepositoryAuthoritySelectionV1::new(
                facts.actor_binding().id(),
                facts.actor_session().id(),
                ordinary_grant_id,
            ),
            authenticated_human: RepositoryAuthenticatedHumanV1::new(
                facts.responder_binding().id(),
                facts.responder_session().id(),
                facts.responder_session().request_commitment().as_bytes(),
            )
            .unwrap(),
            leaf_authority_expires_at: 190,
        }
    }

    fn continuity_generation(
        manifest: &AuthorityContinuityManifestV1,
        context_id: AuthorityContextIdV1,
    ) -> (
        AuthorityContinuityClosureV1,
        AdmittedTransitionGuardV1,
        SuccessVisibleAuthorityContinuityStateV1,
    ) {
        let accepted_time = AcceptedAuthorityTimeFloorV1::context_genesis(
            reference("stage3-stable-lineage"),
            reference("stage3-trusted-time-coordinate"),
            reference("stage3-trusted-time-stack"),
            reference("stage3-trusted-time-origin"),
            120,
            130,
        )
        .unwrap();
        let allocation = StoreAllocatedContinuityStateTokenV1::from_store_commitments(
            context_id,
            1,
            None,
            1,
            digest("stage3-state-token"),
            digest("stage3-allocation"),
        )
        .unwrap();
        let semantic_cut = AuthorityContinuitySemanticCutV1 {
            cut_sequence: 1,
            source_store_generation: 0,
            successor_store_generation: 1,
            authority_epoch: 7,
            stable_lineage: reference("stage3-stable-lineage"),
            selected_trusted_time_stack: reference("stage3-trusted-time-stack"),
            carrier_profile: ContinuityCarrierProfileStatusV1::Confirmed {
                profile: reference("stage3-carrier-profile"),
                accepted_prefix: reference("stage3-accepted-prefix"),
                handoff_state: reference("stage3-handoff-state"),
                fence: reference("stage3-carrier-fence"),
                currentness: reference("stage3-carrier-currentness"),
            },
            accepted_time,
            lane_state_closure_root: reference("stage3-lane-state-root"),
            source_floor_root: reference("stage3-source-floor-root"),
            gap_companions: vec![],
            floor_provenance: vec![],
            external_revision_cells: vec![],
            cma_remaining_root: reference("stage3-cma-remaining"),
            cma_spent_root: reference("stage3-cma-spent"),
            canonical_records: vec![reference("stage3-canonical-record")],
            graph_nodes: vec![],
            replay_items: vec![],
            historical_spend_items: vec![],
            unresolved_effects: vec![],
        };
        let closure = AuthorityContinuityClosureV1::prove(
            manifest,
            AuthorityContinuityClosureInputV1 {
                manifest_id: manifest.id(),
                context_kind: manifest.context_kind(),
                context_id,
                predecessor: AuthorityContinuityPredecessorV1::ContextGenesis {
                    origin_commitment: reference("stage3-context-genesis-origin"),
                },
                class_entries: continuity_class_entries(manifest, &semantic_cut),
                semantic_cut,
                graph_edges: vec![],
                protocol_version: 1,
            },
            &allocation,
        )
        .unwrap();
        let census = TransitionGuardOwnerCensusV1::externally_rooted_genesis(
            context_id,
            1,
            7,
            reference("stage3-context-genesis-origin"),
        )
        .unwrap();
        let guard = AdmittedTransitionGuardV1::evaluate(AuthorityTransitionGuardAdmissionInputV1 {
            kind: GuardAdmissionKindV1::ExternallyRootedContextGenesis,
            context_kind: manifest.context_kind(),
            context_id,
            store_generation: 1,
            authority_epoch: 7,
            manifest_id: manifest.id(),
            closure_id: closure.id(),
            predecessor_state_token: None,
            cut_sequence: 1,
            selected_trusted_time_stack: closure.selected_trusted_time_stack(),
            carrier_profile: closure.carrier_profile().clone(),
            accepted_time: closure.accepted_time().clone(),
            lane_state_closure_root: closure.lane_state_closure_root(),
            source_floor_root: closure.source_floor_root(),
            gap_companions: vec![],
            floor_provenance: vec![],
            external_revision_cells: vec![],
            cma_remaining_root: closure.cma_remaining_root(),
            cma_spent_root: closure.cma_spent_root(),
            unresolved_effects: vec![],
            term_facts: vec![],
            owner_census: census,
            disclosure: ContinuityDisclosureV1::ProtectedComplete,
            protocol_version: 1,
        })
        .unwrap();
        let state =
            SuccessVisibleAuthorityContinuityStateV1::construct(manifest, &closure, &guard, None)
                .unwrap();
        (closure, guard, state)
    }

    fn continuity_class_entries(
        manifest: &AuthorityContinuityManifestV1,
        cut: &AuthorityContinuitySemanticCutV1,
    ) -> Vec<AuthorityContinuityClassClosureV1> {
        let first_canonical = manifest
            .descriptors()
            .iter()
            .find(|descriptor| descriptor.disposition == ClassDispositionV1::CanonicalRecordClosure)
            .map(|descriptor| descriptor.class_id)
            .unwrap();
        manifest
            .descriptors()
            .iter()
            .map(|descriptor| {
                let facets = ContinuityClosureFacetV1::ALL
                    .into_iter()
                    .map(|facet| {
                        let disposition = match descriptor.disposition {
                            ClassDispositionV1::CanonicalRecordClosure => {
                                let items = if descriptor.class_id == first_canonical {
                                    match facet {
                                        ContinuityClosureFacetV1::CanonicalRecords => {
                                            cut.canonical_records.clone()
                                        }
                                        ContinuityClosureFacetV1::Graph => cut.graph_nodes.clone(),
                                        ContinuityClosureFacetV1::Replay => {
                                            cut.replay_items.clone()
                                        }
                                        ContinuityClosureFacetV1::HistoricalSpend => {
                                            cut.historical_spend_items.clone()
                                        }
                                        ContinuityClosureFacetV1::UnresolvedEffect => {
                                            cut.unresolved_effects.clone()
                                        }
                                    }
                                } else {
                                    vec![]
                                };
                                ClosureFacetDispositionKindV1::ContributesExactRoot(
                                    ContinuityExactRootV1::new(
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                        items,
                                    )
                                    .unwrap(),
                                )
                            }
                            ClassDispositionV1::DerivedOnly => {
                                ClosureFacetDispositionKindV1::DerivedCheck {
                                    invariant: class_facet_reference(
                                        "invariant",
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                    ),
                                    proof: class_facet_reference(
                                        "proof",
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                    ),
                                }
                            }
                        };
                        AuthorityContinuityFacetDispositionV1 { facet, disposition }
                    })
                    .collect();
                AuthorityContinuityClassClosureV1 {
                    class_id: descriptor.class_id,
                    owner: descriptor.owner,
                    facets,
                }
            })
            .collect()
    }

    fn class_facet_reference(
        purpose: &str,
        class_id: ContinuityClassIdV1,
        facet: ContinuityClosureFacetV1,
        cut_sequence: u64,
    ) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::from_digest(
            hash(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.continuity-class-facet-proof.v1").unwrap(),
                CborValue::text(purpose).unwrap(),
                class_id.schema_value(),
                CborValue::Unsigned(facet as u64),
                CborValue::Unsigned(cut_sequence),
            ]))
            .unwrap(),
        )
    }

    fn reference(seed: &str) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::derive(seed).unwrap()
    }

    fn digest(seed: &str) -> [u8; 32] {
        Sha256::digest(seed.as_bytes()).into()
    }
}

#[cfg(test)]
mod ancestry_tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::test_support::{AuthorityFixtureModeV1, repository_authority_fixture};
    use super::*;
    use crate::domain::vnext::authority::IdempotencyKeyIdV1;
    use crate::domain::vnext::identity::ContractRootIdV1;
    use crate::domain::vnext::persistence::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreV1,
    };
    use crate::domain::vnext::repository::{
        CancelWorkPublicationV1, CreateDraftWorkPublicationV1, RepositoryActionIdentityV1,
        RepositoryStoreBasisV1, RepositoryStoreV1,
    };
    use crate::domain::vnext::work::{
        WorkIdV1, WorkRecordV1, WorkRecordWriterV1, WorkTransitionReasonV1,
    };

    fn resolve_fixture(
        mode: AuthorityFixtureModeV1,
    ) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
        let fixture = repository_authority_fixture(vec![("CancelWork", [41; 32])], mode);
        resolve_fixture_objects(&fixture)
    }

    fn resolve_fixture_objects(
        fixture: &super::test_support::RepositoryAuthorityFixtureV1,
    ) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
        let snapshot_object = fixture
            .objects
            .iter()
            .find(|object| object.id() == fixture.authority_root_id)
            .unwrap();
        let facts = BootstrapAuthoritySnapshotV1::from_canonical_bytes(
            &object_value_bytes(snapshot_object).unwrap(),
        )
        .unwrap();
        let authority_objects = fixture.objects.iter().collect::<Vec<_>>();
        resolve_repository_authority_chain(
            &facts,
            fixture.selection.terminal_grant_id(),
            &authority_objects,
        )
    }

    #[test]
    fn store_loaded_chain_accepts_g0_through_two_ordinary_grants() {
        let resolved = resolve_fixture(AuthorityFixtureModeV1::MultiHop).unwrap();

        assert_eq!(
            resolved.terminal_grant.grant().id(),
            GrantIdV1::derive("stage3-ordinary-grant").unwrap()
        );
    }

    #[test]
    fn successor_snapshot_preserves_multi_hop_ancestry_for_a_second_repository_admission() {
        let work_id = WorkIdV1::derive("successive-admission-work").unwrap();
        let subject_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-subject.v1").unwrap(),
            bytes(work_id.as_bytes()),
        ]))
        .unwrap();
        let fixture = repository_authority_fixture(
            vec![
                ("CreateDraftWork", subject_commitment),
                ("CancelWork", subject_commitment),
            ],
            AuthorityFixtureModeV1::MultiHop,
        );
        let root = test_root();
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"successive-admission").unwrap();
        let mut store = StoreV1::create(&root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects);
        let contract_root = ContractRootIdV1::parse(&render_digest([55; 32])).unwrap();
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            contract_root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&root);
        let head = store.active_head().unwrap().unwrap();
        let current = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).unwrap();
        let create = CreateDraftWorkPublicationV1::new(
            RepositoryActionIdentityV1::new(
                ActionRequestIdV1::derive("successive-admission-create-request").unwrap(),
                IdempotencyKeyIdV1::derive("successive-admission-create-key").unwrap(),
            ),
            store_basis(&head, &generation),
            fixture.selection,
            work_id,
        )
        .unwrap();
        RepositoryStoreV1::new(&mut store)
            .create_draft_work(create)
            .unwrap();

        let head = store.active_head().unwrap().unwrap();
        let generation = store.publication_generation(head.id()).unwrap();
        let cancel = CancelWorkPublicationV1::new(
            RepositoryActionIdentityV1::new(
                ActionRequestIdV1::derive("successive-admission-cancel-request").unwrap(),
                IdempotencyKeyIdV1::derive("successive-admission-cancel-key").unwrap(),
            ),
            store_basis(&head, &generation),
            fixture.selection,
            current,
            WorkTransitionReasonV1::new("cancel after preserved ancestry").unwrap(),
        )
        .unwrap();
        let outcome = RepositoryStoreV1::new(&mut store)
            .cancel_work(cancel)
            .unwrap();

        assert_eq!(outcome.head().generation_ordinal(), 3);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    fn store_basis(
        head: &crate::domain::vnext::persistence::StoreHeadV1,
        generation: &StoreGenerationV1,
    ) -> RepositoryStoreBasisV1 {
        RepositoryStoreBasisV1::new(
            head.id(),
            generation.id(),
            generation.ordinal(),
            generation.contract_root_id(),
        )
        .unwrap()
    }

    fn put_objects_in_reference_order(store: &mut StoreV1, objects: Vec<StoreObjectV1>) {
        let mut pending = objects;
        let mut inserted = BTreeSet::new();
        while !pending.is_empty() {
            let index = pending
                .iter()
                .position(|object| {
                    object
                        .references()
                        .iter()
                        .all(|reference| inserted.contains(reference))
                })
                .expect("fixture Store objects form a closed DAG");
            let object = pending.remove(index);
            store.put_object(&object).unwrap();
            inserted.insert(object.id());
        }
    }

    fn activate_store(root: &std::path::Path) {
        let connection = Connection::open(root.join("store.sqlite3")).unwrap();
        assert_eq!(
            connection
                .execute(
                    "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                    [],
                )
                .unwrap(),
            1
        );
    }

    fn test_root() -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "maestro-vnext-successive-admission-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    #[test]
    fn store_loaded_chain_rejects_cycle_orphan_cross_root_non_attenuation_staleness_and_revocation()
    {
        for mode in [
            AuthorityFixtureModeV1::MultiHopCycle,
            AuthorityFixtureModeV1::MultiHopExpiredAncestor,
            AuthorityFixtureModeV1::MultiHopForeignRoot,
            AuthorityFixtureModeV1::MultiHopNonAttenuated,
            AuthorityFixtureModeV1::MultiHopRevokedAncestor,
            AuthorityFixtureModeV1::MultiHopStaleRoot,
            AuthorityFixtureModeV1::OrphanedGrant,
        ] {
            assert!(matches!(
                resolve_fixture(mode),
                Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)
                    | Err(RepositoryAuthorityAdmissionErrorV1::AuthorityValidation(_))
            ));
        }
    }

    #[test]
    fn store_loaded_chain_rejects_a_missing_ordinary_ancestor_or_delegation_edge() {
        let parent_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let fixture = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let (parent_object_id, parent) = fixture
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == parent_id)
                    .map(|grant| (object.id(), grant))
            })
            .unwrap();
        let parent_delegation_object_id = fixture
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryGrantDelegationV1::store_schema_id().unwrap()
            })
            .find(|object| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(
                    &object_value_bytes(object).unwrap(),
                    &parent,
                )
                .is_ok()
            })
            .map(StoreObjectV1::id)
            .unwrap();

        for missing_object_id in [parent_object_id, parent_delegation_object_id] {
            let mut missing_ancestor = repository_authority_fixture(
                vec![("CancelWork", [41; 32])],
                AuthorityFixtureModeV1::MultiHop,
            );
            missing_ancestor
                .objects
                .retain(|object| object.id() != missing_object_id);
            assert!(matches!(
                resolve_fixture_objects(&missing_ancestor),
                Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)
            ));
        }
    }

    #[test]
    fn store_loaded_chain_rejects_duplicate_grant_and_delegation_carriers() {
        let mut duplicate_grant = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let parent_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let parent = duplicate_grant
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == parent_id)
            })
            .unwrap();
        let mut duplicate_definition = parent.grant().definition();
        duplicate_definition.grantee_principal_id =
            super::super::super::PrincipalIdV1::derive("stage3-duplicate-parent-principal")
                .unwrap();
        let duplicate_parent =
            OrdinaryBoundedGrantV1::new(duplicate_definition.validate().unwrap()).unwrap();
        duplicate_grant.objects.push(
            authority_object(
                AuthoritySchemaV1::OrdinaryBoundedGrant,
                duplicate_parent.schema_value().unwrap(),
                vec![],
            )
            .unwrap(),
        );
        assert!(resolve_fixture_objects(&duplicate_grant).is_err());

        let mut duplicate_delegation = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let terminal_id = duplicate_delegation.selection.terminal_grant_id();
        let terminal = duplicate_delegation
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == terminal_id)
            })
            .unwrap();
        let duplicate_value = duplicate_delegation
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryGrantDelegationV1::store_schema_id().unwrap()
            })
            .find(|object| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(
                    &object_value_bytes(object).unwrap(),
                    &terminal,
                )
                .is_ok()
            })
            .unwrap()
            .value()
            .clone();
        duplicate_delegation.objects.push(
            authority_object(
                AuthoritySchemaV1::OrdinaryGrantDelegation,
                duplicate_value,
                vec![],
            )
            .unwrap(),
        );
        assert!(resolve_fixture_objects(&duplicate_delegation).is_err());
    }
}
