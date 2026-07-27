//! Stage-8 observation-facing query operation seam.

#![expect(
    dead_code,
    reason = "Stage 8 freezes Observation joins before the Stage 6/7 adapters are integrated"
)]

use thiserror::Error;

use crate::domain::vnext::authority::{AuthorityFacadeV1, ContinuityReferenceV1};
use crate::domain::vnext::capability::runtime::CapabilityViewV1;
use crate::domain::vnext::evidence::diagnostics::{
    OrdinaryDiagnosticViewV1, ProtectedDiagnosticEnvelopeV1,
};
use crate::domain::vnext::intake::IntakeProjectionV1;
use crate::domain::vnext::integration::TrustedHostDiagnosticConnectionPortV1;
use crate::domain::vnext::maturity::MaturityViewV1;
use crate::domain::vnext::memory::MemoryAdvisoryProjectionV1;
use crate::domain::vnext::persistence::ProtectedDiagnosticCurrentViewProviderV1;
use crate::domain::vnext::research::ResearchProjectionV1;
use crate::domain::vnext::search::{SearchProjectionFreshnessV1, SearchProjectionV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InformationSnapshotBindingV1 {
    store_snapshot_ref: [u8; 32],
    projection_ref: [u8; 32],
    planning_assessment_ref: [u8; 32],
    recipe_application_ref: [u8; 32],
}

impl InformationSnapshotBindingV1 {
    pub(crate) fn new(
        store_snapshot_ref: [u8; 32],
        projection_ref: [u8; 32],
        planning_assessment_ref: [u8; 32],
        recipe_application_ref: [u8; 32],
    ) -> Result<Self, InformationObservationErrorV1> {
        if [
            store_snapshot_ref,
            projection_ref,
            planning_assessment_ref,
            recipe_application_ref,
        ]
        .contains(&[0; 32])
        {
            return Err(InformationObservationErrorV1::Unavailable);
        }
        Ok(Self {
            store_snapshot_ref,
            projection_ref,
            planning_assessment_ref,
            recipe_application_ref,
        })
    }

    pub(crate) const fn store_snapshot_ref(self) -> [u8; 32] {
        self.store_snapshot_ref
    }
}

pub(crate) struct InformationObservationInputsV1<'view> {
    pub(crate) binding: InformationSnapshotBindingV1,
    pub(crate) search: &'view SearchProjectionV1,
    pub(crate) memory: &'view MemoryAdvisoryProjectionV1,
    pub(crate) intake: &'view IntakeProjectionV1,
    pub(crate) research: &'view ResearchProjectionV1,
    pub(crate) capability: &'view CapabilityViewV1,
    pub(crate) maturity: &'view MaturityViewV1,
    pub(crate) diagnostics: &'view OrdinaryDiagnosticViewV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InformationObservationV1 {
    binding: InformationSnapshotBindingV1,
    search_hit_count: usize,
    memory_entry_count: usize,
    intake_finding_count: usize,
    research_question_count: usize,
    capability_entry_count: usize,
    maturity_axis_count: usize,
    ordinary_diagnostic_count: usize,
}

impl InformationObservationV1 {
    pub(crate) const fn snapshot_ref(&self) -> [u8; 32] {
        self.binding.store_snapshot_ref
    }
}

pub(crate) fn observe_information(
    inputs: InformationObservationInputsV1<'_>,
) -> Result<InformationObservationV1, InformationObservationErrorV1> {
    let snapshot_ref = inputs.binding.store_snapshot_ref;
    if [
        inputs.search.snapshot_ref(),
        inputs.memory.snapshot_ref(),
        inputs.intake.snapshot_ref(),
        inputs.research.snapshot_ref(),
        inputs.capability.snapshot_ref(),
        inputs.maturity.snapshot_ref(),
        inputs.diagnostics.snapshot_ref(),
    ]
    .iter()
    .any(|candidate| *candidate != snapshot_ref)
        || inputs.search.freshness() != SearchProjectionFreshnessV1::Current
        || inputs.maturity.capability_source_closure_ref() != inputs.capability.source_closure_ref()
    {
        return Err(InformationObservationErrorV1::MixedOrStaleSnapshot);
    }
    Ok(InformationObservationV1 {
        binding: inputs.binding,
        search_hit_count: inputs.search.hits().len(),
        memory_entry_count: inputs.memory.entries().len(),
        intake_finding_count: inputs.intake.findings().len(),
        research_question_count: inputs.research.questions().len(),
        capability_entry_count: inputs.capability.entries().len(),
        maturity_axis_count: inputs.maturity.axes().len(),
        ordinary_diagnostic_count: inputs.diagnostics.entries().len(),
    })
}

pub(crate) fn acquire_protected_continuity_diagnostic(
    authority: &mut AuthorityFacadeV1<'_>,
    connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
) -> Result<ProtectedDiagnosticEnvelopeV1, InformationObservationErrorV1> {
    authority
        .protected_continuity_diagnostic_with_ports(
            connection,
            current_view_provider,
            requested_subject,
        )
        .map(ProtectedDiagnosticEnvelopeV1::from_authority_release)
        .map_err(|_| InformationObservationErrorV1::Unavailable)
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum InformationObservationErrorV1 {
    #[error("information observation unavailable")]
    Unavailable,
    #[error("information observation requires one coherent current snapshot")]
    MixedOrStaleSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::capability::runtime::{
        CapabilitySourceFactV1, CapabilitySourceOwnerV1, CapabilitySubjectV1, build_capability_view,
    };
    use crate::domain::vnext::evidence::diagnostics::project_ordinary_diagnostics;
    use crate::domain::vnext::intake::IntakeLedgerV1;
    use crate::domain::vnext::maturity::build_maturity_view;
    use crate::domain::vnext::memory::MemoryLedgerV1;
    use crate::domain::vnext::research::ResearchLedgerV1;
    use crate::domain::vnext::search::{SearchIndexGenerationV1, SearchQueryV1, project_search};

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn observation_joins_every_information_view_at_one_snapshot() {
        let snapshot = digest(1);
        let search_index =
            SearchIndexGenerationV1::rebuild(snapshot, digest(2), digest(3), []).unwrap();
        let search_query = SearchQueryV1::new(snapshot, ["absent".to_owned()], [], 1).unwrap();
        let search = project_search(&search_index, &search_query);
        let memory = MemoryLedgerV1::empty(snapshot)
            .unwrap()
            .advisory_projection([])
            .unwrap();
        let intake = IntakeLedgerV1::empty(snapshot).unwrap().projection();
        let research = ResearchLedgerV1::empty(snapshot).unwrap().projection();
        let capability = build_capability_view(
            snapshot,
            [CapabilitySourceFactV1::declared(
                snapshot,
                CapabilitySubjectV1::Action(1),
                CapabilitySourceOwnerV1::Contract,
                digest(4),
                digest(5),
                true,
            )
            .unwrap()],
        )
        .unwrap();
        let maturity = build_maturity_view(snapshot, &capability, []).unwrap();
        let diagnostics = project_ordinary_diagnostics(snapshot, digest(6), []).unwrap();
        let observation = observe_information(InformationObservationInputsV1 {
            binding: InformationSnapshotBindingV1::new(snapshot, digest(7), digest(8), digest(9))
                .unwrap(),
            search: &search,
            memory: &memory,
            intake: &intake,
            research: &research,
            capability: &capability,
            maturity: &maturity,
            diagnostics: &diagnostics,
        })
        .unwrap();
        assert_eq!(observation.snapshot_ref(), snapshot);
        assert_eq!(observation.maturity_axis_count, 8);
    }

    #[test]
    fn stale_search_projection_refuses_the_whole_join() {
        let snapshot = digest(1);
        let search_index =
            SearchIndexGenerationV1::rebuild(snapshot, digest(2), digest(3), []).unwrap();
        let search_query = SearchQueryV1::new(digest(9), ["absent".to_owned()], [], 1).unwrap();
        let search = project_search(&search_index, &search_query);
        let memory = MemoryLedgerV1::empty(snapshot)
            .unwrap()
            .advisory_projection([])
            .unwrap();
        let intake = IntakeLedgerV1::empty(snapshot).unwrap().projection();
        let research = ResearchLedgerV1::empty(snapshot).unwrap().projection();
        let capability = build_capability_view(
            snapshot,
            [CapabilitySourceFactV1::declared(
                snapshot,
                CapabilitySubjectV1::Action(1),
                CapabilitySourceOwnerV1::Contract,
                digest(4),
                digest(5),
                true,
            )
            .unwrap()],
        )
        .unwrap();
        let maturity = build_maturity_view(snapshot, &capability, []).unwrap();
        let diagnostics = project_ordinary_diagnostics(snapshot, digest(6), []).unwrap();

        assert_eq!(
            observe_information(InformationObservationInputsV1 {
                binding: InformationSnapshotBindingV1::new(
                    snapshot,
                    digest(7),
                    digest(8),
                    digest(9),
                )
                .unwrap(),
                search: &search,
                memory: &memory,
                intake: &intake,
                research: &research,
                capability: &capability,
                maturity: &maturity,
                diagnostics: &diagnostics,
            })
            .unwrap_err(),
            InformationObservationErrorV1::MixedOrStaleSnapshot
        );
    }

    #[test]
    fn mismatched_capability_closure_refuses_the_whole_join() {
        let snapshot = digest(1);
        let search_index =
            SearchIndexGenerationV1::rebuild(snapshot, digest(2), digest(3), []).unwrap();
        let search_query = SearchQueryV1::new(snapshot, ["absent".to_owned()], [], 1).unwrap();
        let search = project_search(&search_index, &search_query);
        let memory = MemoryLedgerV1::empty(snapshot)
            .unwrap()
            .advisory_projection([])
            .unwrap();
        let intake = IntakeLedgerV1::empty(snapshot).unwrap().projection();
        let research = ResearchLedgerV1::empty(snapshot).unwrap().projection();
        let capability = build_capability_view(
            snapshot,
            [CapabilitySourceFactV1::declared(
                snapshot,
                CapabilitySubjectV1::Action(1),
                CapabilitySourceOwnerV1::Contract,
                digest(4),
                digest(5),
                true,
            )
            .unwrap()],
        )
        .unwrap();
        let different_capability = build_capability_view(
            snapshot,
            [CapabilitySourceFactV1::declared(
                snapshot,
                CapabilitySubjectV1::Action(1),
                CapabilitySourceOwnerV1::Contract,
                digest(40),
                digest(50),
                true,
            )
            .unwrap()],
        )
        .unwrap();
        let maturity = build_maturity_view(snapshot, &different_capability, []).unwrap();
        let diagnostics = project_ordinary_diagnostics(snapshot, digest(6), []).unwrap();

        assert_eq!(
            observe_information(InformationObservationInputsV1 {
                binding: InformationSnapshotBindingV1::new(
                    snapshot,
                    digest(7),
                    digest(8),
                    digest(9),
                )
                .unwrap(),
                search: &search,
                memory: &memory,
                intake: &intake,
                research: &research,
                capability: &capability,
                maturity: &maturity,
                diagnostics: &diagnostics,
            })
            .unwrap_err(),
            InformationObservationErrorV1::MixedOrStaleSnapshot
        );
    }
}
