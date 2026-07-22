#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the sealed diagnostic port without a production provider"
    )
)]

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::domain::vnext::identity::{
    ContractRootIdV1, StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1,
};

use super::{StoreDomainV1, StoreGenerationV1, StoreHeadV1, StoreRoleV1};

const ANCHOR_DOMAIN_V1: &[u8] = b"maestro.vnext.protected-diagnostic-current-view-anchor.v1";

pub(crate) struct ProtectedDiagnosticCurrentViewAnchorV1<'view> {
    commitment: [u8; 32],
    initial_view: ProtectedDiagnosticOwnedCurrentViewV1,
    provider: Option<&'view mut dyn ProtectedDiagnosticCurrentViewProviderV1>,
    provider_binding: Option<ProtectedDiagnosticProviderBindingV1>,
    _view_root: &'view Path,
    completed: bool,
}

impl<'view> ProtectedDiagnosticCurrentViewAnchorV1<'view> {
    pub(super) fn bind(
        provider: &'view mut dyn ProtectedDiagnosticCurrentViewProviderV1,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
        view_root: &'view Path,
    ) -> Option<Self> {
        let provider_currentness = provider.bind_current_view(observed)?;
        let Some(provider_binding) = ProtectedDiagnosticProviderBindingV1::from_bound_provider(
            observed,
            provider_currentness,
        ) else {
            provider.abandon_current_view();
            return None;
        };
        Some(Self {
            commitment: provider_binding.anchor_commitment,
            initial_view: ProtectedDiagnosticOwnedCurrentViewV1::from_observed(observed),
            provider: Some(provider),
            provider_binding: Some(provider_binding),
            _view_root: view_root,
            completed: false,
        })
    }

    pub(crate) const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub(super) fn consume_final_recheck(
        mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> bool {
        let unchanged_view = self.initial_view.matches(observed);
        let Some(provider) = self.provider.take() else {
            return false;
        };
        let Some(binding) = self.provider_binding.take() else {
            provider.abandon_current_view();
            self.completed = true;
            return false;
        };
        let valid = if unchanged_view {
            provider.final_recheck_current_view(&binding.provider_currentness, observed)
        } else {
            provider.abandon_current_view();
            false
        };
        self.completed = true;
        valid
    }
}

impl Drop for ProtectedDiagnosticCurrentViewAnchorV1<'_> {
    fn drop(&mut self) {
        if !self.completed {
            if let Some(provider) = self.provider.take() {
                provider.abandon_current_view();
            }
            self.provider_binding.take();
            self.completed = true;
        }
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

struct ProtectedDiagnosticOwnedCurrentViewV1 {
    root_commitment: [u8; 32],
    state_revision: u64,
    domain_id: StoreDomainIdV1,
    role: StoreRoleV1,
    head_id: StoreHeadIdV1,
    head_revision: u64,
    generation_id: StoreGenerationIdV1,
    generation_ordinal: u64,
    contract_root_id: ContractRootIdV1,
    publication_clock: u64,
}

impl ProtectedDiagnosticOwnedCurrentViewV1 {
    fn from_observed(observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>) -> Self {
        Self {
            root_commitment: path_commitment(observed.root),
            state_revision: observed.state_revision,
            domain_id: observed.domain.id(),
            role: observed.role,
            head_id: observed.head.id(),
            head_revision: observed.head_revision,
            generation_id: observed.generation.id(),
            generation_ordinal: observed.generation.ordinal(),
            contract_root_id: observed.generation.contract_root_id(),
            publication_clock: observed.publication_clock,
        }
    }

    fn matches(&self, observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>) -> bool {
        self.root_commitment == path_commitment(observed.root)
            && self.state_revision == observed.state_revision
            && self.domain_id == observed.domain.id()
            && self.role == observed.role
            && self.head_id == observed.head.id()
            && self.head_revision == observed.head_revision
            && self.generation_id == observed.generation.id()
            && self.generation_ordinal == observed.generation.ordinal()
            && self.contract_root_id == observed.generation.contract_root_id()
            && self.publication_clock == observed.publication_clock
    }
}

pub(crate) struct ProtectedDiagnosticProviderBindingV1 {
    anchor_commitment: [u8; 32],
    provider_currentness: ProtectedDiagnosticProviderCurrentnessV1,
}

impl ProtectedDiagnosticProviderBindingV1 {
    fn from_bound_provider(
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
        provider_currentness: ProtectedDiagnosticProviderCurrentnessV1,
    ) -> Option<Self> {
        let anchor_commitment =
            protected_diagnostic_anchor_commitment(observed, &provider_currentness);
        if anchor_commitment == [0; 32] || provider_currentness.commitment() == [0; 32] {
            return None;
        }
        Some(Self {
            anchor_commitment,
            provider_currentness,
        })
    }
}

pub(crate) struct ProtectedDiagnosticProviderCurrentnessV1 {
    store_instance_binding: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_token: [u8; 32],
    activation_carrier_revision: u64,
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    activation_restore_incarnation: [u8; 32],
    provider_currentness_revision: u64,
}

impl ProtectedDiagnosticProviderCurrentnessV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the currentness port binds every locked activation and restore dimension"
    )]
    pub(crate) fn from_live_provider(
        store_instance_binding: [u8; 32],
        activation_carrier_identity: [u8; 32],
        activation_carrier_token: [u8; 32],
        activation_carrier_revision: u64,
        activation_attempt_identity: [u8; 32],
        activation_destination_seal: [u8; 32],
        activation_restore_incarnation: [u8; 32],
        provider_currentness_revision: u64,
    ) -> Option<Self> {
        if [
            store_instance_binding,
            activation_carrier_identity,
            activation_carrier_token,
            activation_attempt_identity,
            activation_destination_seal,
            activation_restore_incarnation,
        ]
        .contains(&[0; 32])
            || activation_carrier_revision == 0
            || provider_currentness_revision == 0
        {
            return None;
        }
        Some(Self {
            store_instance_binding,
            activation_carrier_identity,
            activation_carrier_token,
            activation_carrier_revision,
            activation_attempt_identity,
            activation_destination_seal,
            activation_restore_incarnation,
            provider_currentness_revision,
        })
    }

    fn commitment(&self) -> [u8; 32] {
        provider_currentness_commitment(
            &[
                self.store_instance_binding,
                self.activation_carrier_identity,
                self.activation_carrier_token,
                self.activation_attempt_identity,
                self.activation_destination_seal,
                self.activation_restore_incarnation,
            ],
            self.activation_carrier_revision,
            self.provider_currentness_revision,
        )
    }

    fn matches(&self, other: &Self) -> bool {
        self.store_instance_binding == other.store_instance_binding
            && self.activation_carrier_identity == other.activation_carrier_identity
            && self.activation_carrier_token == other.activation_carrier_token
            && self.activation_carrier_revision == other.activation_carrier_revision
            && self.activation_attempt_identity == other.activation_attempt_identity
            && self.activation_destination_seal == other.activation_destination_seal
            && self.activation_restore_incarnation == other.activation_restore_incarnation
            && self.provider_currentness_revision == other.provider_currentness_revision
    }
}

mod sealed {
    pub(crate) trait Sealed {}
}

pub(crate) use sealed::Sealed as ProtectedDiagnosticCurrentViewProviderSealedV1;

pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1: sealed::Sealed {
    fn bind_current_view(
        &mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticProviderCurrentnessV1>;

    fn final_recheck_current_view(
        &mut self,
        initial_currentness: &ProtectedDiagnosticProviderCurrentnessV1,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> bool;

    fn abandon_current_view(&mut self);
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
    ProviderCurrentnessRevision,
}

#[cfg(test)]
impl ProtectedDiagnosticTestAnchorMutationV1 {
    pub(crate) const ALL: [Self; 18] = [
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
        Self::ProviderCurrentnessRevision,
    ];
}

#[cfg(test)]
enum ProtectedDiagnosticTestFinalRecheckMutationV1 {
    Substitute(ProtectedDiagnosticTestAnchorMutationV1),
    Aba(ProtectedDiagnosticTestAnchorMutationV1),
    Unavailable,
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
    provider_currentness_revision: u64,
    sealed_provider_commitment: [u8; 32],
    final_recheck_mutation: Option<ProtectedDiagnosticTestFinalRecheckMutationV1>,
    bound: bool,
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
        let provider_currentness_revision = 1;
        let sealed_provider_commitment = provider_currentness_commitment(
            &[
                store_instance_binding,
                activation_carrier_identity,
                activation_carrier_token,
                activation_attempt_identity,
                activation_destination_seal,
                activation_restore_incarnation,
            ],
            activation_carrier_revision,
            provider_currentness_revision,
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
            provider_currentness_revision,
            sealed_provider_commitment,
            final_recheck_mutation: None,
            bound: false,
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
            ProtectedDiagnosticTestAnchorMutationV1::ProviderCurrentnessRevision => {
                self.provider_currentness_revision =
                    self.provider_currentness_revision.saturating_add(1)
            }
        }
    }

    pub(crate) fn substitute_on_final_recheck(
        &mut self,
        mutation: ProtectedDiagnosticTestAnchorMutationV1,
    ) {
        self.final_recheck_mutation = Some(
            ProtectedDiagnosticTestFinalRecheckMutationV1::Substitute(mutation),
        );
    }

    pub(crate) fn aba_on_final_recheck(
        &mut self,
        mutation: ProtectedDiagnosticTestAnchorMutationV1,
    ) {
        self.final_recheck_mutation =
            Some(ProtectedDiagnosticTestFinalRecheckMutationV1::Aba(mutation));
    }

    pub(crate) fn unavailable_on_final_recheck(&mut self) {
        self.final_recheck_mutation =
            Some(ProtectedDiagnosticTestFinalRecheckMutationV1::Unavailable);
    }

    pub(crate) const fn was_consumed(&self) -> bool {
        self.consumed
    }

    fn matches_observed(&self, observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>) -> bool {
        self.expected_root_commitment == path_commitment(observed.root)
            && self.expected_domain_id == observed.domain.id()
            && self.expected_role == observed.role
            && self.expected_head_id == observed.head.id()
            && self.expected_head_revision == observed.head_revision
            && self.expected_generation_id == observed.generation.id()
            && self.expected_generation_ordinal == observed.generation.ordinal()
            && self.expected_contract_root_id == observed.generation.contract_root_id()
            && self.expected_state_revision == observed.state_revision
            && self.expected_publication_clock == observed.publication_clock
    }

    fn current_provider_commitment(&self) -> [u8; 32] {
        self.current_provider_currentness()
            .map_or([0; 32], |currentness| currentness.commitment())
    }

    fn current_provider_currentness(&self) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
        ProtectedDiagnosticProviderCurrentnessV1::from_live_provider(
            self.store_instance_binding,
            self.activation_carrier_identity,
            self.activation_carrier_token,
            self.activation_carrier_revision,
            self.activation_attempt_identity,
            self.activation_destination_seal,
            self.activation_restore_incarnation,
            self.provider_currentness_revision,
        )
    }

    fn apply_currentness_mutation(&mut self, mutation: ProtectedDiagnosticTestAnchorMutationV1) {
        self.substitute_anchor_dimension(mutation);
        if !matches!(
            mutation,
            ProtectedDiagnosticTestAnchorMutationV1::ProviderCurrentnessRevision
        ) {
            self.provider_currentness_revision =
                self.provider_currentness_revision.saturating_add(1);
        }
    }

    fn cycle_currentness_dimension(&mut self, mutation: ProtectedDiagnosticTestAnchorMutationV1) {
        match mutation {
            ProtectedDiagnosticTestAnchorMutationV1::StoreInstance => {
                mutate_digest(&mut self.store_instance_binding);
                mutate_digest(&mut self.store_instance_binding);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierIdentity => {
                mutate_digest(&mut self.activation_carrier_identity);
                mutate_digest(&mut self.activation_carrier_identity);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierToken => {
                mutate_digest(&mut self.activation_carrier_token);
                mutate_digest(&mut self.activation_carrier_token);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationCarrierRevision => {
                let original = self.activation_carrier_revision;
                self.activation_carrier_revision = original.saturating_add(1);
                self.activation_carrier_revision = original;
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationAttempt => {
                mutate_digest(&mut self.activation_attempt_identity);
                mutate_digest(&mut self.activation_attempt_identity);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationDestinationSeal => {
                mutate_digest(&mut self.activation_destination_seal);
                mutate_digest(&mut self.activation_destination_seal);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ActivationRestoreIncarnation => {
                mutate_digest(&mut self.activation_restore_incarnation);
                mutate_digest(&mut self.activation_restore_incarnation);
            }
            ProtectedDiagnosticTestAnchorMutationV1::ProviderCurrentnessRevision => {}
            _ => unreachable!("test fixture schedules only provider-owned currentness dimensions"),
        }
        self.provider_currentness_revision = self.provider_currentness_revision.saturating_add(2);
    }
}

#[cfg(test)]
impl sealed::Sealed for ProtectedDiagnosticTestCurrentViewProviderV1 {}

#[cfg(test)]
impl ProtectedDiagnosticCurrentViewProviderV1 for ProtectedDiagnosticTestCurrentViewProviderV1 {
    fn bind_current_view(
        &mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
        if self.bound
            || self.consumed
            || !self.matches_observed(observed)
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
            || self.provider_currentness_revision == 0
            || self.sealed_provider_commitment != self.current_provider_commitment()
        {
            self.consumed = true;
            return None;
        }
        self.bound = true;
        self.current_provider_currentness()
    }

    fn final_recheck_current_view(
        &mut self,
        initial_currentness: &ProtectedDiagnosticProviderCurrentnessV1,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> bool {
        if !self.bound || self.consumed {
            self.consumed = true;
            return false;
        }
        match self.final_recheck_mutation.take() {
            Some(ProtectedDiagnosticTestFinalRecheckMutationV1::Substitute(mutation)) => {
                self.apply_currentness_mutation(mutation)
            }
            Some(ProtectedDiagnosticTestFinalRecheckMutationV1::Aba(mutation)) => {
                let before = self.current_provider_commitment();
                self.cycle_currentness_dimension(mutation);
                debug_assert_ne!(before, self.current_provider_commitment());
            }
            Some(ProtectedDiagnosticTestFinalRecheckMutationV1::Unavailable) => {
                self.bound = false;
                self.consumed = true;
                return false;
            }
            None => {}
        }
        let current_currentness = self.current_provider_currentness();
        let valid = self.matches_observed(observed)
            && current_currentness
                .as_ref()
                .is_some_and(|currentness| initial_currentness.matches(currentness))
            && self.sealed_provider_commitment == self.current_provider_commitment();
        self.bound = false;
        self.consumed = true;
        valid
    }

    fn abandon_current_view(&mut self) {
        self.bound = false;
        self.consumed = true;
    }
}

fn protected_diagnostic_anchor_commitment(
    observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    provider: &ProtectedDiagnosticProviderCurrentnessV1,
) -> [u8; 32] {
    let root_commitment = path_commitment(observed.root);
    let state_revision = observed.state_revision.to_be_bytes();
    let role = observed.role.tag().to_be_bytes();
    let head_revision = observed.head_revision.to_be_bytes();
    let generation_ordinal = observed.generation.ordinal().to_be_bytes();
    let publication_clock = observed.publication_clock.to_be_bytes();
    let activation_revision = provider.activation_carrier_revision.to_be_bytes();
    let currentness_revision = provider.provider_currentness_revision.to_be_bytes();
    tuple_commitment(
        ANCHOR_DOMAIN_V1,
        &[
            &root_commitment,
            &provider.store_instance_binding,
            &state_revision,
            &role,
            observed.domain.id().as_bytes(),
            observed.head.id().as_bytes(),
            &head_revision,
            observed.generation.id().as_bytes(),
            &generation_ordinal,
            observed.generation.contract_root_id().as_bytes(),
            &publication_clock,
            &provider.activation_carrier_identity,
            &provider.activation_carrier_token,
            &activation_revision,
            &provider.activation_attempt_identity,
            &provider.activation_destination_seal,
            &provider.activation_restore_incarnation,
            &currentness_revision,
        ],
    )
}

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

fn provider_currentness_commitment(
    identities: &[[u8; 32]; 6],
    activation_carrier_revision: u64,
    provider_currentness_revision: u64,
) -> [u8; 32] {
    tuple_commitment(
        b"maestro.vnext.protected-diagnostic-provider-currentness.v1",
        &[
            &identities[0],
            &identities[1],
            &identities[2],
            &activation_carrier_revision.to_be_bytes(),
            &identities[3],
            &identities[4],
            &identities[5],
            &provider_currentness_revision.to_be_bytes(),
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
