use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::distribution::runtime::{
    CutoverPlanOwnerFactsV1, DistributionDomainKindV1, DistributionMutationKindV1,
    DistributionPlanTargetV1, DistributionPlanV1, DistributionTransactionPhaseV1,
    DistributionTransactionV1, TargetCustodyClassV1, TargetEffectKindV1,
};
use crate::domain::distribution::{CommitmentV1, ReleaseIdV1};
use crate::domain::installation::{
    CommittedAgentResourceReleaseV1, RepositoryInstallationClosureV1,
};
use crate::domain::persistence::{StoreRoleV1, StoreStateV1, StoreV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryBootstrapTargetKindV1 {
    MaestroBootstrapFile,
    AgentsManagedPointer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the filesystem descriptor provider supplies one frozen explicit authorization"
    )
)]
pub(crate) enum RepositoryBootstrapAuthorizationV1 {
    Apply,
    Force,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapTargetFactsV1 {
    target_identity: CommitmentV1,
    expected_preimage: CommitmentV1,
    shown_diff: CommitmentV1,
    authorization: RepositoryBootstrapAuthorizationV1,
    outside_prefix: Option<CommitmentV1>,
    outside_suffix: Option<CommitmentV1>,
}

impl RepositoryBootstrapTargetFactsV1 {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the filesystem descriptor provider constructs owner facts at the cutover assembly boundary"
        )
    )]
    pub(crate) fn new(
        target_identity: CommitmentV1,
        expected_preimage: CommitmentV1,
        shown_diff: CommitmentV1,
        authorization: RepositoryBootstrapAuthorizationV1,
        outside_prefix: Option<CommitmentV1>,
        outside_suffix: Option<CommitmentV1>,
    ) -> Result<Self, RepositoryBootstrapErrorV1> {
        if [target_identity, expected_preimage, shown_diff]
            .iter()
            .any(|value| value.as_bytes() == &[0; 32])
            || outside_prefix.is_some_and(|value| value.as_bytes() == &[0; 32])
            || outside_suffix.is_some_and(|value| value.as_bytes() == &[0; 32])
            || outside_prefix.is_some() != outside_suffix.is_some()
        {
            return Err(RepositoryBootstrapErrorV1::InvalidOwnerFacts);
        }
        Ok(Self {
            target_identity,
            expected_preimage,
            shown_diff,
            authorization,
            outside_prefix,
            outside_suffix,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapOwnerFactsV1 {
    plan: CutoverPlanOwnerFactsV1<2>,
    maestro_file: RepositoryBootstrapTargetFactsV1,
    agents_managed_pointer: RepositoryBootstrapTargetFactsV1,
}

impl RepositoryBootstrapOwnerFactsV1 {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Repository owner provider constructs admitted bootstrap facts"
        )
    )]
    pub(crate) fn new(
        plan: CutoverPlanOwnerFactsV1<2>,
        maestro_file: RepositoryBootstrapTargetFactsV1,
        agents_managed_pointer: RepositoryBootstrapTargetFactsV1,
    ) -> Result<Self, RepositoryBootstrapErrorV1> {
        if plan.domain.kind() != DistributionDomainKindV1::RepositoryDomain
            || plan.target_custodies[0].class() != TargetCustodyClassV1::MaestroOwnedTarget
            || plan.target_custodies[1].class() != TargetCustodyClassV1::SharedManagedBlock
            || maestro_file.target_identity == agents_managed_pointer.target_identity
            || maestro_file.outside_prefix.is_some()
            || agents_managed_pointer.outside_prefix.is_none()
        {
            return Err(RepositoryBootstrapErrorV1::InvalidOwnerFacts);
        }
        Ok(Self {
            plan,
            maestro_file,
            agents_managed_pointer,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositoryBootstrapBindingV1 {
    kind: RepositoryBootstrapTargetKindV1,
    target_tag: u64,
    target_identity: CommitmentV1,
    expected_preimage: CommitmentV1,
    candidate: CommitmentV1,
    shown_diff: CommitmentV1,
    authorization: RepositoryBootstrapAuthorizationV1,
    effect_kind: TargetEffectKindV1,
    custody: TargetCustodyClassV1,
    outside_prefix: Option<CommitmentV1>,
    outside_suffix: Option<CommitmentV1>,
}

#[derive(Debug)]
pub(crate) struct RepositoryBootstrapAdmissionV1 {
    installation_release_id: ReleaseIdV1,
    installation_result_closure: [u8; 32],
    reconnect_closure: [u8; 32],
    bootstrap_closure: [u8; 32],
    bindings: [RepositoryBootstrapBindingV1; 2],
    plan: DistributionPlanV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RepositoryBootstrapAdmissionV1 {
    pub(crate) fn after_agent_resource_release(
        release: CommittedAgentResourceReleaseV1,
        owner_facts: RepositoryBootstrapOwnerFactsV1,
    ) -> Result<Self, RepositoryBootstrapErrorV1> {
        let bindings = owner_derived_bindings(&owner_facts);
        let plan = build_plan(&owner_facts.plan, &bindings)?;
        let bootstrap_closure = repository_bootstrap_closure(
            release.release_id(),
            release.installation_result_closure(),
            release.reconnect_closure(),
            &bindings,
        );
        Ok(Self {
            installation_release_id: release.release_id(),
            installation_result_closure: release.installation_result_closure(),
            reconnect_closure: release.reconnect_closure(),
            bootstrap_closure,
            bindings,
            plan,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn validate_plan(
        &self,
        plan: &DistributionPlanV1,
    ) -> Result<(), RepositoryBootstrapErrorV1> {
        if plan != &self.plan
            || self.bootstrap_closure
                != repository_bootstrap_closure(
                    self.installation_release_id,
                    self.installation_result_closure,
                    self.reconnect_closure,
                    &self.bindings,
                )
        {
            return Err(RepositoryBootstrapErrorV1::PlanMismatch);
        }
        Ok(())
    }

    pub(crate) fn plan(&self) -> &DistributionPlanV1 {
        &self.plan
    }

    pub(crate) fn authorize_effects(
        &self,
        observations: [RepositoryBootstrapEffectObservationV1; 2],
    ) -> Result<RepositoryBootstrapEffectPermitV1, RepositoryBootstrapErrorV1> {
        self.validate_plan(&self.plan)?;
        if !self
            .bindings
            .iter()
            .zip(observations)
            .all(|(binding, observed)| {
                observed.target_identity == binding.target_identity
                    && observed.expected_preimage == binding.expected_preimage
                    && observed.shown_diff == binding.shown_diff
                    && observed.authorization == binding.authorization
            })
        {
            return Err(RepositoryBootstrapErrorV1::NotAuthorized);
        }
        Ok(RepositoryBootstrapEffectPermitV1 {
            bootstrap_closure: self.bootstrap_closure,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn validate_effect_permit(
        &self,
        permit: &RepositoryBootstrapEffectPermitV1,
    ) -> Result<(), RepositoryBootstrapErrorV1> {
        self.validate_plan(&self.plan)?;
        if permit.bootstrap_closure != self.bootstrap_closure {
            return Err(RepositoryBootstrapErrorV1::NotAuthorized);
        }
        Ok(())
    }

    pub(crate) fn acquire_coherent_readback(
        &self,
        store: &StoreV1,
        closure: &RepositoryInstallationClosureV1,
        reader: &mut dyn RepositoryBootstrapDescriptorReadPortV1,
    ) -> Result<RepositoryBootstrapReadbackV1, RepositoryBootstrapErrorV1> {
        self.validate_plan(&self.plan)?;
        let before = repository_snapshot_binding(store, closure)?;
        let observations = reader.read_exact_targets(&self.plan)?;
        let after = repository_snapshot_binding(store, closure)?;
        if before != after {
            return Err(RepositoryBootstrapErrorV1::NotCurrent);
        }
        self.validated_readback(observations)
    }

    fn validated_readback(
        &self,
        observations: [RepositoryBootstrapDescriptorObservationV1; 2],
    ) -> Result<RepositoryBootstrapReadbackV1, RepositoryBootstrapErrorV1> {
        let targets = observations.map(|observed| RepositoryBootstrapTargetReadbackV1 {
            target_identity: observed.target_identity,
            candidate: observed.content_sha256,
            outside_prefix: observed.outside_prefix,
            outside_suffix: observed.outside_suffix,
        });
        let readback = RepositoryBootstrapReadbackV1 { targets };
        self.validate_readback(&readback)?;
        Ok(readback)
    }

    fn validate_readback(
        &self,
        readback: &RepositoryBootstrapReadbackV1,
    ) -> Result<(), RepositoryBootstrapErrorV1> {
        if !self
            .bindings
            .iter()
            .zip(readback.targets)
            .all(|(binding, observed)| {
                observed.target_identity == binding.target_identity
                    && observed.candidate == binding.candidate
                    && observed.outside_prefix == binding.outside_prefix
                    && observed.outside_suffix == binding.outside_suffix
            })
        {
            return Err(RepositoryBootstrapErrorV1::NotCurrent);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapEffectObservationV1 {
    pub target_identity: CommitmentV1,
    pub expected_preimage: CommitmentV1,
    pub shown_diff: CommitmentV1,
    pub authorization: RepositoryBootstrapAuthorizationV1,
}

#[derive(Debug)]
pub(crate) struct RepositoryBootstrapEffectPermitV1 {
    bootstrap_closure: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapDescriptorObservationV1 {
    pub target_identity: CommitmentV1,
    pub content_sha256: CommitmentV1,
    pub outside_prefix: Option<CommitmentV1>,
    pub outside_suffix: Option<CommitmentV1>,
}

pub(crate) trait RepositoryBootstrapDescriptorReadPortV1 {
    fn read_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<[RepositoryBootstrapDescriptorObservationV1; 2], RepositoryBootstrapErrorV1>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositoryBootstrapTargetReadbackV1 {
    target_identity: CommitmentV1,
    candidate: CommitmentV1,
    outside_prefix: Option<CommitmentV1>,
    outside_suffix: Option<CommitmentV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapReadbackV1 {
    targets: [RepositoryBootstrapTargetReadbackV1; 2],
}

#[derive(Debug)]
pub(crate) struct CommittedRepositoryBootstrapV1 {
    repository_result_closure: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CommittedRepositoryBootstrapV1 {
    pub(crate) fn confirm(
        admission: RepositoryBootstrapAdmissionV1,
        transaction: &DistributionTransactionV1,
        closure: &RepositoryInstallationClosureV1,
        readback: &RepositoryBootstrapReadbackV1,
    ) -> Result<Self, RepositoryBootstrapErrorV1> {
        admission.validate_plan(transaction.plan())?;
        admission.validate_readback(readback)?;
        closure
            .validate()
            .map_err(|_| RepositoryBootstrapErrorV1::NotCurrent)?;
        if transaction.phase() != DistributionTransactionPhaseV1::Committed
            || closure.domain != *transaction.plan().domain()
        {
            return Err(RepositoryBootstrapErrorV1::NotCurrent);
        }
        let closure_id = closure
            .to_store_object()
            .map_err(|_| RepositoryBootstrapErrorV1::NotCurrent)?
            .id();
        let repository_result_closure = commitment(
            b"maestro.vnext.repository-bootstrap-result.v1",
            &[
                *admission.installation_release_id.as_bytes(),
                admission.installation_result_closure,
                admission.reconnect_closure,
                admission.bootstrap_closure,
                *transaction.plan().meaning_digest().as_bytes(),
                *closure_id.as_bytes(),
                readback_closure(readback),
            ],
        );
        Ok(Self {
            repository_result_closure,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn repository_result_closure(&self) -> [u8; 32] {
        self.repository_result_closure
    }
}

const MAESTRO_BOOTSTRAP_BYTES_V1: &[u8] =
    include_bytes!("../../../embedded/vnext/bootstrap/MAESTRO.md");
const AGENTS_MANAGED_POINTER_BYTES_V1: &[u8] = b"<!-- maestro:start -->\n\
# Maestro Harness Protocol\n\
Read .maestro/MAESTRO.md first before working in this repo.\n\
<!-- maestro:end -->\n";

fn owner_derived_bindings(
    facts: &RepositoryBootstrapOwnerFactsV1,
) -> [RepositoryBootstrapBindingV1; 2] {
    [
        (
            RepositoryBootstrapTargetKindV1::MaestroBootstrapFile,
            facts.maestro_file,
            MAESTRO_BOOTSTRAP_BYTES_V1,
            TargetEffectKindV1::RewriteOwnedTarget,
            TargetCustodyClassV1::MaestroOwnedTarget,
        ),
        (
            RepositoryBootstrapTargetKindV1::AgentsManagedPointer,
            facts.agents_managed_pointer,
            AGENTS_MANAGED_POINTER_BYTES_V1,
            TargetEffectKindV1::RewriteManagedBlock,
            TargetCustodyClassV1::SharedManagedBlock,
        ),
    ]
    .map(
        |(kind, facts, bytes, effect_kind, custody)| RepositoryBootstrapBindingV1 {
            kind,
            target_tag: kind as u64 + 1,
            target_identity: facts.target_identity,
            expected_preimage: facts.expected_preimage,
            candidate: CommitmentV1::from_bytes(Sha256::digest(bytes).into()),
            shown_diff: facts.shown_diff,
            authorization: facts.authorization,
            effect_kind,
            custody,
            outside_prefix: facts.outside_prefix,
            outside_suffix: facts.outside_suffix,
        },
    )
}

fn build_plan(
    facts: &CutoverPlanOwnerFactsV1<2>,
    bindings: &[RepositoryBootstrapBindingV1; 2],
) -> Result<DistributionPlanV1, RepositoryBootstrapErrorV1> {
    let targets = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| DistributionPlanTargetV1 {
            target_tag: binding.target_tag,
            target_identity_ref: facts.target_identity_refs[index].clone(),
            target_identity: binding.target_identity,
            custody: facts.target_custodies[index].clone(),
            expected_preimage_commitment: binding.expected_preimage,
            candidate_commitment: Some(binding.candidate),
            effect_kind: binding.effect_kind,
            outside_prefix_commitment: binding.outside_prefix,
            outside_suffix_commitment: binding.outside_suffix,
        })
        .collect();
    DistributionPlanV1::new(
        facts.domain.clone(),
        DistributionMutationKindV1::Migrate,
        facts.request_id,
        facts.request_or_ceremony_ref.clone(),
        facts.plan_ref.clone(),
        facts.idempotency_key_ref.clone(),
        None,
        facts.prior_commit_ref.clone(),
        facts.prior_receipt_ref.clone(),
        None,
        targets,
    )
    .map_err(|_| RepositoryBootstrapErrorV1::InvalidOwnerFacts)
}

fn binding_parts(binding: &RepositoryBootstrapBindingV1) -> Vec<[u8; 32]> {
    vec![
        digest_u64(binding.kind as u64 + 1),
        digest_u64(binding.target_tag),
        *binding.target_identity.as_bytes(),
        *binding.expected_preimage.as_bytes(),
        *binding.candidate.as_bytes(),
        *binding.shown_diff.as_bytes(),
        digest_u64(binding.authorization as u64 + 1),
        digest_u64(binding.effect_kind.numeric_tag()),
        digest_u64(binding.custody.numeric_tag()),
        binding
            .outside_prefix
            .map_or([0; 32], |value| *value.as_bytes()),
        binding
            .outside_suffix
            .map_or([0; 32], |value| *value.as_bytes()),
    ]
}

fn repository_bootstrap_closure(
    release_id: ReleaseIdV1,
    installation_result_closure: [u8; 32],
    reconnect_closure: [u8; 32],
    bindings: &[RepositoryBootstrapBindingV1; 2],
) -> [u8; 32] {
    commitment(
        b"maestro.vnext.repository-bootstrap.v1",
        &[
            vec![
                *release_id.as_bytes(),
                installation_result_closure,
                reconnect_closure,
            ],
            bindings.iter().flat_map(binding_parts).collect::<Vec<_>>(),
        ]
        .concat(),
    )
}

fn readback_closure(readback: &RepositoryBootstrapReadbackV1) -> [u8; 32] {
    let parts = readback
        .targets
        .iter()
        .flat_map(|target| {
            [
                *target.target_identity.as_bytes(),
                *target.candidate.as_bytes(),
                target
                    .outside_prefix
                    .map_or([0; 32], |value| *value.as_bytes()),
                target
                    .outside_suffix
                    .map_or([0; 32], |value| *value.as_bytes()),
            ]
        })
        .collect::<Vec<_>>();
    commitment(b"maestro.vnext.repository-bootstrap-readback.v1", &parts)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositorySnapshotBindingV1 {
    head_id: crate::domain::identity::StoreHeadIdV1,
    generation_id: crate::domain::identity::StoreGenerationIdV1,
    closure_object_id: crate::domain::identity::StoreObjectIdV1,
}

fn repository_snapshot_binding(
    store: &StoreV1,
    closure: &RepositoryInstallationClosureV1,
) -> Result<RepositorySnapshotBindingV1, RepositoryBootstrapErrorV1> {
    let (state, head, generation, objects) = store
        .coherent_publication_snapshot()
        .map_err(|_| RepositoryBootstrapErrorV1::NotCurrent)?;
    let closure_object = closure
        .to_store_object()
        .map_err(|_| RepositoryBootstrapErrorV1::NotCurrent)?;
    if state != StoreStateV1::Active
        || store.role() != StoreRoleV1::Repository
        || !objects.iter().any(|object| object == &closure_object)
    {
        return Err(RepositoryBootstrapErrorV1::NotCurrent);
    }
    Ok(RepositorySnapshotBindingV1 {
        head_id: head.id(),
        generation_id: generation.id(),
        closure_object_id: closure_object.id(),
    })
}

fn digest_u64(value: u64) -> [u8; 32] {
    Sha256::digest(value.to_be_bytes()).into()
}

fn commitment(domain: &[u8], parts: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum RepositoryBootstrapErrorV1 {
    #[error("the Repository bootstrap owner facts are incomplete or invalid")]
    InvalidOwnerFacts,
    #[error("the Distribution plan does not match the exact Repository bootstrap")]
    PlanMismatch,
    #[error("the Repository bootstrap diff or explicit authorization is missing")]
    NotAuthorized,
    #[error("the Repository bootstrap descriptor read failed: {0}")]
    DescriptorRead(String),
    #[error("the Repository bootstrap is not committed under exact Repository currentness")]
    NotCurrent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::authority::ActionRequestIdV1;
    use crate::domain::distribution::runtime::{
        CustodyAssessmentV1, CustodyBasisV1, DistributionDomainRefV1,
        DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1, ManagedBlockBoundaryV1,
    };
    use crate::domain::identity::StoreObjectIdV1;

    fn value(byte: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([byte; 32])
    }

    fn object(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
    }

    fn scoped(
        domain: &DistributionDomainRefV1,
        kind: DistributionRuntimeObjectKindV1,
        byte: u8,
    ) -> DistributionScopedObjectRefV1 {
        DistributionScopedObjectRefV1::new(domain.clone(), kind, object(byte)).unwrap()
    }

    fn owner_facts() -> RepositoryBootstrapOwnerFactsV1 {
        let domain = DistributionDomainRefV1::new(
            DistributionDomainKindV1::RepositoryDomain,
            value(1),
            value(2),
            value(3),
        )
        .unwrap();
        let identities = [value(20), value(21)];
        let custodies = std::array::from_fn(|index| {
            CustodyAssessmentV1::assess(&CustodyBasisV1 {
                domain: domain.clone(),
                target_identity: identities[index],
                alias_closure_id: value(30 + index as u8),
                receipt_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::DistributionReceipt,
                    40 + index as u8,
                )),
                claim_ref: Some(scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                    50 + index as u8,
                )),
                claimed_target_identity: Some(identities[index]),
                resource_id: Some(value(60 + index as u8)),
                bundle_id: Some(value(70 + index as u8)),
                release_id: Some(value(80 + index as u8)),
                claimed_content_sha256: Some(value(90 + index as u8)),
                observed_content_sha256: Some(value(90 + index as u8)),
                managed_block: (index == 1).then(|| ManagedBlockBoundaryV1 {
                    start_marker: b"<!-- maestro:start -->".to_vec(),
                    end_marker: b"<!-- maestro:end -->".to_vec(),
                    block_sha256: value(100),
                    outside_prefix_sha256: value(101),
                    outside_suffix_sha256: value(102),
                }),
                foreign_owner_observed: false,
                external_manager_observed: false,
                alias_ambiguous: false,
                unsafe_path_state: false,
            })
            .unwrap()
        });
        let plan = CutoverPlanOwnerFactsV1::new(
            domain.clone(),
            ActionRequestIdV1::derive("repository-bootstrap-test").unwrap(),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                4,
            ),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionPlan,
                5,
            ),
            scoped(&domain, DistributionRuntimeObjectKindV1::IdempotencyKey, 6),
            None,
            None,
            std::array::from_fn(|index| {
                scoped(
                    &domain,
                    DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                    7 + index as u8,
                )
            }),
            custodies,
        );
        RepositoryBootstrapOwnerFactsV1::new(
            plan,
            RepositoryBootstrapTargetFactsV1::new(
                identities[0],
                value(110),
                value(111),
                RepositoryBootstrapAuthorizationV1::Apply,
                None,
                None,
            )
            .unwrap(),
            RepositoryBootstrapTargetFactsV1::new(
                identities[1],
                value(112),
                value(113),
                RepositoryBootstrapAuthorizationV1::Force,
                Some(value(101)),
                Some(value(102)),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn admission() -> RepositoryBootstrapAdmissionV1 {
        RepositoryBootstrapAdmissionV1::after_agent_resource_release(
            CommittedAgentResourceReleaseV1::test_committed(value(120), [121; 32], [122; 32]),
            owner_facts(),
        )
        .unwrap()
    }

    #[test]
    fn repository_owner_binds_installation_result_preflight_and_exact_readback() {
        let repository_admission = admission();
        assert_eq!(repository_admission.plan().release_id(), None);
        assert_eq!(
            repository_admission.plan().domain().kind(),
            DistributionDomainKindV1::RepositoryDomain
        );

        let observations =
            repository_admission
                .bindings
                .map(|binding| RepositoryBootstrapEffectObservationV1 {
                    target_identity: binding.target_identity,
                    expected_preimage: binding.expected_preimage,
                    shown_diff: binding.shown_diff,
                    authorization: binding.authorization,
                });
        assert!(repository_admission.authorize_effects(observations).is_ok());
        let mut wrong_diff = observations;
        wrong_diff[0].shown_diff = value(123);
        assert!(matches!(
            repository_admission.authorize_effects(wrong_diff),
            Err(RepositoryBootstrapErrorV1::NotAuthorized)
        ));

        let readback = repository_admission
            .validated_readback(repository_admission.bindings.map(|binding| {
                RepositoryBootstrapDescriptorObservationV1 {
                    target_identity: binding.target_identity,
                    content_sha256: binding.candidate,
                    outside_prefix: binding.outside_prefix,
                    outside_suffix: binding.outside_suffix,
                }
            }))
            .unwrap();
        assert_eq!(repository_admission.validate_readback(&readback), Ok(()));
        let mut wrong_targets = readback.targets;
        wrong_targets[1].outside_suffix = Some(value(124));
        assert_eq!(
            repository_admission.validate_readback(&RepositoryBootstrapReadbackV1 {
                targets: wrong_targets
            }),
            Err(RepositoryBootstrapErrorV1::NotCurrent)
        );

        let mut binding_tamper = admission();
        binding_tamper.bindings[0].shown_diff = value(125);
        assert_eq!(
            binding_tamper.validate_plan(binding_tamper.plan()),
            Err(RepositoryBootstrapErrorV1::PlanMismatch)
        );

        let mut closure_tamper = admission();
        closure_tamper.bootstrap_closure = [0; 32];
        assert_eq!(
            closure_tamper.validate_plan(closure_tamper.plan()),
            Err(RepositoryBootstrapErrorV1::PlanMismatch)
        );
    }
}
