#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the sealed diagnostic port without a production provider"
    )
)]

use std::path::Path;

use sha2::{Digest, Sha256};

#[cfg(test)]
use crate::domain::vnext::identity::{
    ContractRootIdV1, StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1,
};

use super::{StoreDomainV1, StoreGenerationV1, StoreHeadV1, StoreRoleV1};

const ANCHOR_DOMAIN_V1: &[u8] = b"maestro.vnext.protected-diagnostic-current-view-anchor.v1";

pub(crate) struct ProtectedDiagnosticCurrentViewAnchorV1 {
    commitment: [u8; 32],
}

impl ProtectedDiagnosticCurrentViewAnchorV1 {
    pub(crate) const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }
}

pub(crate) struct ProtectedDiagnosticObservedCurrentViewV1<'view> {
    pub(crate) root: &'view Path,
    pub(crate) state_revision: u64,
    pub(crate) domain: &'view StoreDomainV1,
    pub(crate) role: StoreRoleV1,
    pub(crate) head: &'view StoreHeadV1,
    pub(crate) head_revision: u64,
    pub(crate) generation: &'view StoreGenerationV1,
    pub(crate) publication_clock: u64,
}

mod sealed {
    pub trait Sealed {}
}

pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1: sealed::Sealed {
    fn bind_current_view(
        &mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticCurrentViewAnchorV1>;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum ProtectedDiagnosticTestAnchorMutationV1 {
    Root,
    Domain,
    Role,
    StateRevision,
    Head,
    HeadRevision,
    Generation,
    GenerationOrdinal,
    ContractRoot,
    PublicationClock,
    StoreInstance,
    ActivationCarrierIdentity,
    ActivationCarrierToken,
    ActivationCarrierRevision,
    ActivationAttempt,
    ActivationDestinationSeal,
    ActivationRestoreIncarnation,
}

#[cfg(test)]
impl ProtectedDiagnosticTestAnchorMutationV1 {
    pub(crate) const ALL: [Self; 17] = [
        Self::Root,
        Self::Domain,
        Self::Role,
        Self::StateRevision,
        Self::Head,
        Self::HeadRevision,
        Self::Generation,
        Self::GenerationOrdinal,
        Self::ContractRoot,
        Self::PublicationClock,
        Self::StoreInstance,
        Self::ActivationCarrierIdentity,
        Self::ActivationCarrierToken,
        Self::ActivationCarrierRevision,
        Self::ActivationAttempt,
        Self::ActivationDestinationSeal,
        Self::ActivationRestoreIncarnation,
    ];
}

#[cfg(test)]
pub(crate) struct ProtectedDiagnosticTestCurrentViewProviderV1 {
    expected_root_commitment: [u8; 32],
    expected_domain_id: StoreDomainIdV1,
    expected_role: StoreRoleV1,
    expected_head_id: StoreHeadIdV1,
    expected_head_revision: u64,
    expected_generation_id: StoreGenerationIdV1,
    expected_generation_ordinal: u64,
    expected_contract_root_id: ContractRootIdV1,
    expected_state_revision: u64,
    expected_publication_clock: u64,
    store_instance_binding: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_token: [u8; 32],
    activation_carrier_revision: u64,
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    activation_restore_incarnation: [u8; 32],
    sealed_provider_commitment: [u8; 32],
    consumed: bool,
}

#[cfg(test)]
impl ProtectedDiagnosticTestCurrentViewProviderV1 {
    pub(crate) fn bound_to(
        root: &Path,
        domain: &StoreDomainV1,
        head: &StoreHeadV1,
        generation: &StoreGenerationV1,
        state_revision: u64,
        publication_clock: u64,
        seed: &[u8],
    ) -> Option<Self> {
        if seed.is_empty() || state_revision == 0 || publication_clock == 0 {
            return None;
        }
        let derive = |label: &[u8]| tuple_commitment(label, &[seed]);
        let store_instance_binding = derive(b"store-instance");
        let activation_carrier_identity = derive(b"activation-carrier");
        let activation_carrier_token = derive(b"activation-token");
        let activation_carrier_revision = 1;
        let activation_attempt_identity = derive(b"activation-attempt");
        let activation_destination_seal = derive(b"destination-seal");
        let activation_restore_incarnation = derive(b"activation-restore-incarnation");
        let sealed_provider_commitment = provider_currentness_commitment(
            store_instance_binding,
            activation_carrier_identity,
            activation_carrier_token,
            activation_carrier_revision,
            activation_attempt_identity,
            activation_destination_seal,
            activation_restore_incarnation,
        );
        Some(Self {
            expected_root_commitment: path_commitment(root),
            expected_domain_id: domain.id(),
            expected_role: domain.role(),
            expected_head_id: head.id(),
            expected_head_revision: head.revision(),
            expected_generation_id: generation.id(),
            expected_generation_ordinal: generation.ordinal(),
            expected_contract_root_id: generation.contract_root_id(),
            expected_state_revision: state_revision,
            expected_publication_clock: publication_clock,
            store_instance_binding,
            activation_carrier_identity,
            activation_carrier_token,
            activation_carrier_revision,
            activation_attempt_identity,
            activation_destination_seal,
            activation_restore_incarnation,
            sealed_provider_commitment,
            consumed: false,
        })
    }

    pub(crate) fn substitute_anchor_dimension(
        &mut self,
        mutation: ProtectedDiagnosticTestAnchorMutationV1,
    ) {
        match mutation {
            ProtectedDiagnosticTestAnchorMutationV1::Root => {
                mutate_digest(&mut self.expected_root_commitment)
            }
            ProtectedDiagnosticTestAnchorMutationV1::Domain => {
                self.expected_domain_id = StoreDomainIdV1::from_digest([0xa1; 32])
            }
            ProtectedDiagnosticTestAnchorMutationV1::Role => {
                self.expected_role = StoreRoleV1::Installation
            }
            ProtectedDiagnosticTestAnchorMutationV1::StateRevision => {
                self.expected_state_revision = self.expected_state_revision.saturating_add(1)
            }
            ProtectedDiagnosticTestAnchorMutationV1::Head => {
                self.expected_head_id = StoreHeadIdV1::from_digest([0xa2; 32])
            }
            ProtectedDiagnosticTestAnchorMutationV1::HeadRevision => {
                self.expected_head_revision = self.expected_head_revision.saturating_add(1)
            }
            ProtectedDiagnosticTestAnchorMutationV1::Generation => {
                self.expected_generation_id = StoreGenerationIdV1::from_digest([0xa3; 32])
            }
            ProtectedDiagnosticTestAnchorMutationV1::GenerationOrdinal => {
                self.expected_generation_ordinal =
                    self.expected_generation_ordinal.saturating_add(1)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ContractRoot => {
                self.expected_contract_root_id = ContractRootIdV1::from_digest([0xa4; 32])
            }
            ProtectedDiagnosticTestAnchorMutationV1::PublicationClock => {
                self.expected_publication_clock = self.expected_publication_clock.saturating_add(1)
            }
            ProtectedDiagnosticTestAnchorMutationV1::StoreInstance => {
                mutate_digest(&mut self.store_instance_binding)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierIdentity => {
                mutate_digest(&mut self.activation_carrier_identity)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierToken => {
                mutate_digest(&mut self.activation_carrier_token)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierRevision => {
                self.activation_carrier_revision =
                    self.activation_carrier_revision.saturating_add(1)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationAttempt => {
                mutate_digest(&mut self.activation_attempt_identity)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationDestinationSeal => {
                mutate_digest(&mut self.activation_destination_seal)
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationRestoreIncarnation => {
                mutate_digest(&mut self.activation_restore_incarnation)
            }
        }
    }
}

#[cfg(test)]
impl sealed::Sealed for ProtectedDiagnosticTestCurrentViewProviderV1 {}

#[cfg(test)]
impl ProtectedDiagnosticCurrentViewProviderV1 for ProtectedDiagnosticTestCurrentViewProviderV1 {
    fn bind_current_view(
        &mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticCurrentViewAnchorV1> {
        if self.consumed
            || self.expected_root_commitment != path_commitment(observed.root)
            || self.expected_domain_id != observed.domain.id()
            || self.expected_role != observed.role
            || self.expected_head_id != observed.head.id()
            || self.expected_head_revision != observed.head_revision
            || self.expected_generation_id != observed.generation.id()
            || self.expected_generation_ordinal != observed.generation.ordinal()
            || self.expected_contract_root_id != observed.generation.contract_root_id()
            || self.expected_state_revision != observed.state_revision
            || self.expected_publication_clock != observed.publication_clock
            || [
                self.store_instance_binding,
                self.activation_carrier_identity,
                self.activation_carrier_token,
                self.activation_attempt_identity,
                self.activation_destination_seal,
                self.activation_restore_incarnation,
            ]
            .contains(&[0; 32])
            || self.activation_carrier_revision == 0
            || self.sealed_provider_commitment
                != provider_currentness_commitment(
                    self.store_instance_binding,
                    self.activation_carrier_identity,
                    self.activation_carrier_token,
                    self.activation_carrier_revision,
                    self.activation_attempt_identity,
                    self.activation_destination_seal,
                    self.activation_restore_incarnation,
                )
        {
            self.consumed = true;
            return None;
        }
        self.consumed = true;
        let state_revision = observed.state_revision.to_be_bytes();
        let role = observed.role.tag().to_be_bytes();
        let head_revision = observed.head_revision.to_be_bytes();
        let generation_ordinal = observed.generation.ordinal().to_be_bytes();
        let publication_clock = observed.publication_clock.to_be_bytes();
        let activation_revision = self.activation_carrier_revision.to_be_bytes();
        let commitment = tuple_commitment(
            ANCHOR_DOMAIN_V1,
            &[
                &self.expected_root_commitment,
                &self.store_instance_binding,
                &state_revision,
                &role,
                observed.domain.id().as_bytes(),
                observed.head.id().as_bytes(),
                &head_revision,
                observed.generation.id().as_bytes(),
                &generation_ordinal,
                observed.generation.contract_root_id().as_bytes(),
                &publication_clock,
                &self.activation_carrier_identity,
                &self.activation_carrier_token,
                &activation_revision,
                &self.activation_attempt_identity,
                &self.activation_destination_seal,
                &self.activation_restore_incarnation,
            ],
        );
        Some(ProtectedDiagnosticCurrentViewAnchorV1 { commitment })
    }
}

#[cfg(test)]
fn path_commitment(path: &Path) -> [u8; 32] {
    tuple_commitment(
        b"maestro.vnext.protected-diagnostic-store-root.v1",
        &[path.as_os_str().as_encoded_bytes()],
    )
}

#[cfg(test)]
fn mutate_digest(value: &mut [u8; 32]) {
    value[0] ^= 0xff;
}

#[cfg(test)]
fn provider_currentness_commitment(
    store_instance_binding: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_token: [u8; 32],
    activation_carrier_revision: u64,
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    activation_restore_incarnation: [u8; 32],
) -> [u8; 32] {
    tuple_commitment(
        b"maestro.vnext.protected-diagnostic-provider-currentness.v1",
        &[
            &store_instance_binding,
            &activation_carrier_identity,
            &activation_carrier_token,
            &activation_carrier_revision.to_be_bytes(),
            &activation_attempt_identity,
            &activation_destination_seal,
            &activation_restore_incarnation,
        ],
    )
}

fn tuple_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}
