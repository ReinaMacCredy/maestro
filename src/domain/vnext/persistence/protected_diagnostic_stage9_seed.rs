use super::protected_diagnostic::{
    ProtectedDiagnosticCurrentViewProviderV1, ProtectedDiagnosticObservedCurrentViewV1,
    ProtectedDiagnosticProviderCurrentnessV1, sealed,
};

struct Stage9OwnerLocalCurrentViewProviderSeedV1 {
    live_store_instance: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_token: [u8; 32],
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    activation_restore_incarnation: [u8; 32],
    activation_carrier_revision: u64,
    provider_currentness_revision: u64,
    bound: bool,
}

impl sealed::Sealed for Stage9OwnerLocalCurrentViewProviderSeedV1 {}

impl ProtectedDiagnosticCurrentViewProviderV1 for Stage9OwnerLocalCurrentViewProviderSeedV1 {
    fn bind_current_view(
        &mut self,
        _observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
        if self.bound {
            return None;
        }
        self.bound = true;
        ProtectedDiagnosticProviderCurrentnessV1::from_live_provider(
            self.live_store_instance,
            self.activation_carrier_identity,
            self.activation_carrier_token,
            self.activation_carrier_revision,
            self.activation_attempt_identity,
            self.activation_destination_seal,
            self.activation_restore_incarnation,
            self.provider_currentness_revision,
        )
    }

    fn final_recheck_current_view(
        &mut self,
        initial_currentness: &ProtectedDiagnosticProviderCurrentnessV1,
        _observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> bool {
        let current = ProtectedDiagnosticProviderCurrentnessV1::from_live_provider(
            self.live_store_instance,
            self.activation_carrier_identity,
            self.activation_carrier_token,
            self.activation_carrier_revision,
            self.activation_attempt_identity,
            self.activation_destination_seal,
            self.activation_restore_incarnation,
            self.provider_currentness_revision,
        );
        let valid = self.bound
            && current
                .as_ref()
                .is_some_and(|current| initial_currentness.matches(current));
        self.bound = false;
        valid
    }

    fn abandon_current_view(&mut self) {
        self.bound = false;
    }
}

#[test]
fn stage9_owner_local_descendant_can_mint_only_structured_live_currentness() {
    fn require_provider<T: ProtectedDiagnosticCurrentViewProviderV1>() {}
    require_provider::<Stage9OwnerLocalCurrentViewProviderSeedV1>();
}
