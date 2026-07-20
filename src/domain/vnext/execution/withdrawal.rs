use thiserror::Error;

use super::effect_home::EffectIntentHomeKindV1;

pub const WITHDRAWN_LOCALLY_RENDERING_V1: &str =
    "withdrawn locally; no provider cancellation performed";
pub const WITHDRAWAL_ROUTE_CATALOG_IDENTITY_V1: &str =
    "sha256:261ffce2f96126b5339ceb74197f2f59c8c51a5fad7c4a01d6ed1ef04e7e5c1f";
pub const WITHDRAWAL_SEMANTIC_BINDING_V1: &str =
    "exact_effect_intent_origin_semantic_subject_and_origination_fence";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentLiveDispatchV1 {
    None,
    Reserved,
    Sealed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteClassificationV1 {
    Prepared,
    Dispatching,
    Pending,
    InDoubt,
    ConfirmedApplied,
    ConfirmedNotApplied,
    PartiallyApplied,
    Conflicted,
    Cancelled,
}

impl RemoteClassificationV1 {
    pub const fn withdrawal_source_literal(self) -> Option<&'static str> {
        match self {
            Self::Prepared => Some("prepared"),
            Self::ConfirmedNotApplied => Some("confirmed_not_applied"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectWithdrawalSlotFamilyV1 {
    MaintenanceExecutorCurrentness,
    ProspectiveContinuityCarrier,
    PlannedTurnoverHighWater,
    RepositoryRecoveryAdmission,
    InstallationRecoveryAdmission,
}

impl EffectWithdrawalSlotFamilyV1 {
    pub const ALL: [Self; 5] = [
        Self::MaintenanceExecutorCurrentness,
        Self::ProspectiveContinuityCarrier,
        Self::PlannedTurnoverHighWater,
        Self::RepositoryRecoveryAdmission,
        Self::InstallationRecoveryAdmission,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawalAuthorityPathV1 {
    Ordinary,
    BootstrapG0,
    ContinuityMaintenance(EffectWithdrawalSlotFamilyV1),
    Ceremony,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawalRouteBindingV1 {
    Action { action_tag: u64 },
    Ceremony { ceremony_tag: u8 },
}

impl WithdrawalRouteBindingV1 {
    pub const fn branch_tag(self) -> u64 {
        match self {
            Self::Action { action_tag } => action_tag,
            Self::Ceremony { ceremony_tag } => ceremony_tag as u64,
        }
    }

    pub const fn role_literal(self) -> &'static str {
        match self {
            Self::Action { .. } => "ActionWithdraw",
            Self::Ceremony { .. } => "CeremonyWithdraw",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawalCatalogCellV1 {
    classification: RemoteClassificationV1,
    compatibility_identity: &'static str,
    origin_tag: u8,
    route_tag: u8,
    route: WithdrawalRouteBindingV1,
    home: EffectIntentHomeKindV1,
    catalog_descriptor_id: &'static str,
    semantic_binding: &'static str,
    authority_path: WithdrawalAuthorityPathV1,
}

impl WithdrawalCatalogCellV1 {
    pub const fn classification(self) -> RemoteClassificationV1 {
        self.classification
    }

    pub const fn classification_literal(self) -> &'static str {
        match self.classification.withdrawal_source_literal() {
            Some(literal) => literal,
            None => unreachable!(),
        }
    }

    pub const fn compatibility_identity(self) -> &'static str {
        self.compatibility_identity
    }

    pub const fn origin_tag(self) -> u8 {
        self.origin_tag
    }

    pub const fn route_tag(self) -> u8 {
        self.route_tag
    }

    pub const fn route(self) -> WithdrawalRouteBindingV1 {
        self.route
    }

    pub const fn role_literal(self) -> &'static str {
        self.route.role_literal()
    }

    pub const fn branch_tag(self) -> u64 {
        self.route.branch_tag()
    }

    pub const fn home(self) -> EffectIntentHomeKindV1 {
        self.home
    }

    pub const fn home_literal(self) -> &'static str {
        match self.home {
            EffectIntentHomeKindV1::ActiveStore => "ActiveStoreHomeV1",
            EffectIntentHomeKindV1::NoStoreCeremony => "NoStoreCeremonyHomeV1",
            EffectIntentHomeKindV1::PreStoreCeremony => "PreStoreCeremonyHomeV1",
        }
    }

    pub const fn catalog_descriptor_id(self) -> &'static str {
        self.catalog_descriptor_id
    }

    pub const fn semantic_binding(self) -> &'static str {
        self.semantic_binding
    }

    pub const fn authority_path(self) -> WithdrawalAuthorityPathV1 {
        self.authority_path
    }
}

#[derive(Clone, Copy)]
struct WithdrawalCatalogBaseV1 {
    origin_tag: u8,
    route_tag: u8,
    route: WithdrawalRouteBindingV1,
    home: EffectIntentHomeKindV1,
    catalog_descriptor_id: &'static str,
    authority_path: WithdrawalAuthorityPathV1,
}

const ORDINARY_DESCRIPTOR: &str =
    "e23ecde2a44f067a8e8ca6b8044fc817b1ac66cf26af903f78847f02c83ec37b";
const BOOTSTRAP_DESCRIPTOR: &str =
    "7ef86c95d5c0b6919768a9d85c287e97f3adb3537b0a1e1df9bc9cfe6b427738";
const CONTINUITY_DESCRIPTOR: &str =
    "03bba94f384e4552c0d62b612d32822f6606c27ac13d0fa7b64bf4a35f74e93f";

const fn action_base(
    origin_tag: u8,
    action_tag: u64,
    descriptor: &'static str,
    authority_path: WithdrawalAuthorityPathV1,
) -> WithdrawalCatalogBaseV1 {
    WithdrawalCatalogBaseV1 {
        origin_tag,
        route_tag: 5,
        route: WithdrawalRouteBindingV1::Action { action_tag },
        home: EffectIntentHomeKindV1::ActiveStore,
        catalog_descriptor_id: descriptor,
        authority_path,
    }
}

const fn ceremony_base(
    origin_tag: u8,
    route_tag: u8,
    ceremony_tag: u8,
    home: EffectIntentHomeKindV1,
    descriptor: &'static str,
) -> WithdrawalCatalogBaseV1 {
    WithdrawalCatalogBaseV1 {
        origin_tag,
        route_tag,
        route: WithdrawalRouteBindingV1::Ceremony { ceremony_tag },
        home,
        catalog_descriptor_id: descriptor,
        authority_path: WithdrawalAuthorityPathV1::Ceremony,
    }
}

const WITHDRAWAL_CATALOG_BASES_V1: [WithdrawalCatalogBaseV1; 30] = [
    action_base(
        1,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        2,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        3,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        4,
        37,
        BOOTSTRAP_DESCRIPTOR,
        WithdrawalAuthorityPathV1::BootstrapG0,
    ),
    action_base(
        5,
        37,
        BOOTSTRAP_DESCRIPTOR,
        WithdrawalAuthorityPathV1::BootstrapG0,
    ),
    action_base(
        6,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        7,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        8,
        38,
        CONTINUITY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::ContinuityMaintenance(
            EffectWithdrawalSlotFamilyV1::MaintenanceExecutorCurrentness,
        ),
    ),
    action_base(
        9,
        38,
        CONTINUITY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::ContinuityMaintenance(
            EffectWithdrawalSlotFamilyV1::ProspectiveContinuityCarrier,
        ),
    ),
    action_base(
        10,
        38,
        CONTINUITY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::ContinuityMaintenance(
            EffectWithdrawalSlotFamilyV1::PlannedTurnoverHighWater,
        ),
    ),
    action_base(
        11,
        38,
        CONTINUITY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::ContinuityMaintenance(
            EffectWithdrawalSlotFamilyV1::RepositoryRecoveryAdmission,
        ),
    ),
    action_base(
        12,
        38,
        CONTINUITY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::ContinuityMaintenance(
            EffectWithdrawalSlotFamilyV1::InstallationRecoveryAdmission,
        ),
    ),
    action_base(
        13,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    ceremony_base(
        14,
        4,
        10,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "293a84066daee63560cf1f66e6ab8b22a49e63d4c404d300dd807c861eecd6fc",
    ),
    ceremony_base(
        15,
        4,
        11,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "00802a252941f4d28bd87eca9c070a35f414d81105cce47cc527041454ab6316",
    ),
    action_base(
        16,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        17,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        18,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    action_base(
        19,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    ceremony_base(
        19,
        9,
        9,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "2128f75798074f5c2954f26b76ae38ae2bd0729edd9644567f9bb6723cda05eb",
    ),
    action_base(
        20,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
    ceremony_base(
        20,
        9,
        8,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "4a8acfe64af0e8e7b1b76e3119214444a0110e8ee66571fff8e28d04b9f3fa81",
    ),
    ceremony_base(
        21,
        4,
        2,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "4e41ff2c9391d1611a823f7a21b7141ae5c9624344910e2f0c0169738605728d",
    ),
    ceremony_base(
        21,
        8,
        4,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "00fce3bbb795d5ed59c5c5287922282373d54043cdddba5f830d928f57fa3a67",
    ),
    ceremony_base(
        21,
        12,
        6,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "a167cc16f6b75a77c8209a3cf297fbacadb4cf412a04d67204d09d2a34df11cd",
    ),
    ceremony_base(
        22,
        4,
        1,
        EffectIntentHomeKindV1::NoStoreCeremony,
        "b84549b141c2843cdcaeb7dd7c776af558d50a563967ab7f431de736f621ba4c",
    ),
    ceremony_base(
        22,
        8,
        3,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "07fd1f45d5791bb83435b1f3e4242d7ecb1fcc0b519d2592a5cd5305f4b8144a",
    ),
    ceremony_base(
        22,
        12,
        5,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "b82857862fcbc4933701c895f9a8ed1e3590a055bf4fecfc0bccb9623f593207",
    ),
    ceremony_base(
        22,
        16,
        7,
        EffectIntentHomeKindV1::PreStoreCeremony,
        "32bfc50f6b5cf86d40fdaa49fce1aae695c89cc809b39f9e3a3180b7043d7200",
    ),
    action_base(
        23,
        36,
        ORDINARY_DESCRIPTOR,
        WithdrawalAuthorityPathV1::Ordinary,
    ),
];

pub fn withdrawal_catalog_cells_v1() -> Vec<WithdrawalCatalogCellV1> {
    let mut cells = Vec::with_capacity(60);
    for classification in [
        RemoteClassificationV1::Prepared,
        RemoteClassificationV1::ConfirmedNotApplied,
    ] {
        for base in WITHDRAWAL_CATALOG_BASES_V1 {
            cells.push(WithdrawalCatalogCellV1 {
                classification,
                compatibility_identity: WITHDRAWAL_ROUTE_CATALOG_IDENTITY_V1,
                origin_tag: base.origin_tag,
                route_tag: base.route_tag,
                route: base.route,
                home: base.home,
                catalog_descriptor_id: base.catalog_descriptor_id,
                semantic_binding: WITHDRAWAL_SEMANTIC_BINDING_V1,
                authority_path: base.authority_path,
            });
        }
    }
    cells
}

pub(crate) fn action_withdrawal_catalog_cell_v1(
    classification: RemoteClassificationV1,
    origin_tag: u8,
    action_tag: u64,
    authority_path: WithdrawalAuthorityPathV1,
) -> Result<WithdrawalCatalogCellV1, WithdrawalError> {
    withdrawal_catalog_cells_v1()
        .into_iter()
        .find(|cell| {
            cell.classification == classification
                && cell.origin_tag == origin_tag
                && cell.route_tag == 5
                && cell.route == WithdrawalRouteBindingV1::Action { action_tag }
                && cell.authority_path == authority_path
        })
        .ok_or(WithdrawalError::CatalogCellMismatch)
}

pub fn ceremony_withdrawal_catalog_cell_v1(
    classification: RemoteClassificationV1,
    ceremony_tag: u8,
    home: EffectIntentHomeKindV1,
) -> Result<WithdrawalCatalogCellV1, WithdrawalError> {
    withdrawal_catalog_cells_v1()
        .into_iter()
        .find(|cell| {
            cell.classification == classification
                && cell.route == WithdrawalRouteBindingV1::Ceremony { ceremony_tag }
                && cell.home == home
        })
        .ok_or(WithdrawalError::CatalogCellMismatch)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the exact withdrawal lookup binds every frozen catalog-cell dimension independently"
)]
pub(crate) fn exact_withdrawal_catalog_cell_v1(
    classification: RemoteClassificationV1,
    compatibility_identity: &str,
    origin_tag: u8,
    route_tag: u8,
    branch_tag: u64,
    role_literal: &str,
    home: EffectIntentHomeKindV1,
    catalog_descriptor_id: &str,
    semantic_binding: &str,
) -> Result<WithdrawalCatalogCellV1, WithdrawalError> {
    withdrawal_catalog_cells_v1()
        .into_iter()
        .find(|cell| {
            cell.classification == classification
                && cell.compatibility_identity == compatibility_identity
                && cell.origin_tag == origin_tag
                && cell.route_tag == route_tag
                && cell.branch_tag() == branch_tag
                && cell.role_literal() == role_literal
                && cell.home == home
                && cell.catalog_descriptor_id == catalog_descriptor_id
                && cell.semantic_binding == semantic_binding
        })
        .ok_or(WithdrawalError::CatalogCellMismatch)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawalRequestV1 {
    pub catalog_cell: WithdrawalCatalogCellV1,
    pub home: EffectIntentHomeKindV1,
    pub path: WithdrawalAuthorityPathV1,
    pub live_dispatch: EffectIntentLiveDispatchV1,
    pub classification: RemoteClassificationV1,
    pub has_live_attempt: bool,
    pub has_dispatch_fence: bool,
    pub has_seal: bool,
    pub has_release_capability: bool,
    pub runs_closed: bool,
    pub same_home_current: bool,
    pub authority_current: bool,
    pub capacity_current: bool,
    pub expected_old_head: bool,
    pub expected_old_carrier: bool,
}

impl WithdrawalRequestV1 {
    pub const fn legal_for_catalog_cell(catalog_cell: WithdrawalCatalogCellV1) -> Self {
        Self {
            catalog_cell,
            home: catalog_cell.home,
            path: catalog_cell.authority_path,
            live_dispatch: EffectIntentLiveDispatchV1::None,
            classification: catalog_cell.classification,
            has_live_attempt: false,
            has_dispatch_fence: false,
            has_seal: false,
            has_release_capability: false,
            runs_closed: true,
            same_home_current: true,
            authority_current: true,
            capacity_current: true,
            expected_old_head: true,
            expected_old_carrier: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawalLegalityV1 {
    pub next_live_dispatch: EffectIntentLiveDispatchV1,
    pub next_classification: RemoteClassificationV1,
    pub creates_intent: bool,
    pub creates_attempt: bool,
    pub creates_run: bool,
    pub performs_provider_io: bool,
    pub refunds_or_remints: bool,
}

pub fn validate_withdrawal(
    request: WithdrawalRequestV1,
) -> Result<WithdrawalLegalityV1, WithdrawalError> {
    if !withdrawal_catalog_cells_v1().contains(&request.catalog_cell)
        || request.catalog_cell.classification != request.classification
        || request.catalog_cell.home != request.home
        || request.catalog_cell.authority_path != request.path
    {
        return Err(WithdrawalError::CatalogCellMismatch);
    }
    if request.live_dispatch != EffectIntentLiveDispatchV1::None {
        return Err(WithdrawalError::LiveDispatch);
    }
    if !matches!(
        request.classification,
        RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
    ) {
        return Err(WithdrawalError::Classification);
    }
    if request.has_live_attempt
        || request.has_dispatch_fence
        || request.has_seal
        || request.has_release_capability
    {
        return Err(WithdrawalError::LiveAttemptFenceSealOrCapability);
    }
    if !request.runs_closed {
        return Err(WithdrawalError::OpenRuns);
    }
    if !request.same_home_current || !request.authority_current || !request.capacity_current {
        return Err(WithdrawalError::StaleHomeAuthorityOrCapacity);
    }
    if !request.expected_old_head || !request.expected_old_carrier {
        return Err(WithdrawalError::ExpectedOldMismatch);
    }
    match (request.home, request.path) {
        (
            EffectIntentHomeKindV1::ActiveStore,
            WithdrawalAuthorityPathV1::Ordinary
            | WithdrawalAuthorityPathV1::BootstrapG0
            | WithdrawalAuthorityPathV1::ContinuityMaintenance(_),
        ) => {}
        (
            EffectIntentHomeKindV1::NoStoreCeremony | EffectIntentHomeKindV1::PreStoreCeremony,
            WithdrawalAuthorityPathV1::Ceremony,
        ) => {}
        _ => return Err(WithdrawalError::CrossHomeBasisDonation),
    }
    Ok(WithdrawalLegalityV1 {
        next_live_dispatch: EffectIntentLiveDispatchV1::None,
        next_classification: RemoteClassificationV1::Cancelled,
        creates_intent: false,
        creates_attempt: false,
        creates_run: false,
        performs_provider_io: false,
        refunds_or_remints: false,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawalDeniedProductV1 {
    LiveDispatchReservedOrSealed,
    ClassificationNotPreparedOrConfirmedNotApplied,
    LiveAttemptOrDispatchFenceOrSealOrReleaseCapability,
    OpenRunOrIncompleteRunClosure,
    StaleOrUnknownHomeDomainRealmOrContext,
    WrongOriginSemanticSubjectOrOriginationFence,
    StaleGenerationEpochMaterialOrAuthority,
    WrongActionLeafCeremonyModeOrRouteRole,
    OrdinaryBootstrapCmaOrCeremonyBasisDonation,
    MissingOrSpentCmaEffectWithdrawalSlot,
    SixthCmaPurposeOrCapacityKind,
    MissingCurrentWriterTermOrExpectedOldHead,
    UnknownMixedOrStaleCatalogIdentity,
    NewIntentAttemptRunObservationKeyOrEnvelope,
    RefundRemintTopUpRefillRewindOrSlotReopen,
    ProviderCancellationOrCompensationWording,
    TerminalCancelledReopenReconcileRetryOrRedispatch,
    LateEvidenceControlHeadRewrite,
    RestoreWithoutSameHomeOldWriterFence,
    CrossDomainCloneImportOrUnresolvedCollisionActivation,
    LegacyCancelledLabelWithoutUniqueCompleteH3CausalJoin,
}

impl WithdrawalDeniedProductV1 {
    pub const ALL: [Self; 21] = [
        Self::LiveDispatchReservedOrSealed,
        Self::ClassificationNotPreparedOrConfirmedNotApplied,
        Self::LiveAttemptOrDispatchFenceOrSealOrReleaseCapability,
        Self::OpenRunOrIncompleteRunClosure,
        Self::StaleOrUnknownHomeDomainRealmOrContext,
        Self::WrongOriginSemanticSubjectOrOriginationFence,
        Self::StaleGenerationEpochMaterialOrAuthority,
        Self::WrongActionLeafCeremonyModeOrRouteRole,
        Self::OrdinaryBootstrapCmaOrCeremonyBasisDonation,
        Self::MissingOrSpentCmaEffectWithdrawalSlot,
        Self::SixthCmaPurposeOrCapacityKind,
        Self::MissingCurrentWriterTermOrExpectedOldHead,
        Self::UnknownMixedOrStaleCatalogIdentity,
        Self::NewIntentAttemptRunObservationKeyOrEnvelope,
        Self::RefundRemintTopUpRefillRewindOrSlotReopen,
        Self::ProviderCancellationOrCompensationWording,
        Self::TerminalCancelledReopenReconcileRetryOrRedispatch,
        Self::LateEvidenceControlHeadRewrite,
        Self::RestoreWithoutSameHomeOldWriterFence,
        Self::CrossDomainCloneImportOrUnresolvedCollisionActivation,
        Self::LegacyCancelledLabelWithoutUniqueCompleteH3CausalJoin,
    ];

    pub const fn literal(self) -> &'static str {
        match self {
            Self::LiveDispatchReservedOrSealed => "live_dispatch_reserved_or_sealed",
            Self::ClassificationNotPreparedOrConfirmedNotApplied => {
                "classification_not_prepared_or_confirmed_not_applied"
            }
            Self::LiveAttemptOrDispatchFenceOrSealOrReleaseCapability => {
                "live_attempt_or_dispatch_fence_or_seal_or_release_capability"
            }
            Self::OpenRunOrIncompleteRunClosure => "open_run_or_incomplete_run_closure",
            Self::StaleOrUnknownHomeDomainRealmOrContext => {
                "stale_or_unknown_home_domain_realm_or_context"
            }
            Self::WrongOriginSemanticSubjectOrOriginationFence => {
                "wrong_origin_semantic_subject_or_origination_fence"
            }
            Self::StaleGenerationEpochMaterialOrAuthority => {
                "stale_generation_epoch_material_or_authority"
            }
            Self::WrongActionLeafCeremonyModeOrRouteRole => {
                "wrong_action_leaf_ceremony_mode_or_route_role"
            }
            Self::OrdinaryBootstrapCmaOrCeremonyBasisDonation => {
                "ordinary_bootstrap_cma_or_ceremony_basis_donation"
            }
            Self::MissingOrSpentCmaEffectWithdrawalSlot => {
                "missing_or_spent_cma_effect_withdrawal_slot"
            }
            Self::SixthCmaPurposeOrCapacityKind => "sixth_cma_purpose_or_capacity_kind",
            Self::MissingCurrentWriterTermOrExpectedOldHead => {
                "missing_current_writer_term_or_expected_old_head"
            }
            Self::UnknownMixedOrStaleCatalogIdentity => "unknown_mixed_or_stale_catalog_identity",
            Self::NewIntentAttemptRunObservationKeyOrEnvelope => {
                "new_intent_attempt_run_observation_key_or_envelope"
            }
            Self::RefundRemintTopUpRefillRewindOrSlotReopen => {
                "refund_remint_top_up_refill_rewind_or_slot_reopen"
            }
            Self::ProviderCancellationOrCompensationWording => {
                "provider_cancellation_or_compensation_wording"
            }
            Self::TerminalCancelledReopenReconcileRetryOrRedispatch => {
                "terminal_cancelled_reopen_reconcile_retry_or_redispatch"
            }
            Self::LateEvidenceControlHeadRewrite => "late_evidence_control_head_rewrite",
            Self::RestoreWithoutSameHomeOldWriterFence => {
                "restore_without_same_home_old_writer_fence"
            }
            Self::CrossDomainCloneImportOrUnresolvedCollisionActivation => {
                "cross_domain_clone_import_or_unresolved_collision_activation"
            }
            Self::LegacyCancelledLabelWithoutUniqueCompleteH3CausalJoin => {
                "legacy_cancelled_label_without_unique_complete_h3_causal_join"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WithdrawalError {
    #[error("withdrawal route does not match one exact frozen catalog cell")]
    CatalogCellMismatch,
    #[error("withdrawal requires None live dispatch")]
    LiveDispatch,
    #[error("withdrawal requires prepared or confirmed_not_applied")]
    Classification,
    #[error("withdrawal may not have a live Attempt, fence, seal, or release capability")]
    LiveAttemptFenceSealOrCapability,
    #[error("withdrawal requires a closed Run set")]
    OpenRuns,
    #[error("withdrawal requires current same-Home authority and capacity")]
    StaleHomeAuthorityOrCapacity,
    #[error("withdrawal requires the exact expected-old Head and carrier")]
    ExpectedOldMismatch,
    #[error(
        "withdrawal may not donate an ordinary, Bootstrap, CMA, or Ceremony basis across Homes"
    )]
    CrossHomeBasisDonation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn withdrawal_totality_is_sixty_unique_exact_catalog_cells() {
        let cells = withdrawal_catalog_cells_v1();
        assert_eq!(cells.len(), 60);
        for (index, cell) in cells.iter().enumerate() {
            assert!(!cells[..index].contains(cell));
            assert_legal(WithdrawalRequestV1::legal_for_catalog_cell(*cell));
        }
        assert_eq!(
            cells
                .iter()
                .filter(|cell| matches!(cell.route(), WithdrawalRouteBindingV1::Action { .. }))
                .count(),
            38
        );
        assert_eq!(
            cells
                .iter()
                .filter(|cell| matches!(cell.route(), WithdrawalRouteBindingV1::Ceremony { .. }))
                .count(),
            22
        );
    }

    #[test]
    fn withdrawal_denied_product_catalog_is_exact_and_validation_fails_closed() {
        assert_eq!(WithdrawalDeniedProductV1::ALL.len(), 21);
        let base = WithdrawalRequestV1::legal_for_catalog_cell(withdrawal_catalog_cells_v1()[0]);
        let denied = [
            WithdrawalRequestV1 {
                live_dispatch: EffectIntentLiveDispatchV1::Reserved,
                ..base
            },
            WithdrawalRequestV1 {
                has_live_attempt: true,
                ..base
            },
            WithdrawalRequestV1 {
                runs_closed: false,
                ..base
            },
            WithdrawalRequestV1 {
                same_home_current: false,
                ..base
            },
            WithdrawalRequestV1 {
                authority_current: false,
                ..base
            },
            WithdrawalRequestV1 {
                capacity_current: false,
                ..base
            },
            WithdrawalRequestV1 {
                expected_old_head: false,
                ..base
            },
            WithdrawalRequestV1 {
                path: WithdrawalAuthorityPathV1::Ceremony,
                ..base
            },
        ];
        assert!(
            denied
                .into_iter()
                .all(|request| validate_withdrawal(request).is_err())
        );
    }

    fn assert_legal(request: WithdrawalRequestV1) {
        let outcome = validate_withdrawal(request).unwrap();
        assert_eq!(outcome.next_live_dispatch, EffectIntentLiveDispatchV1::None);
        assert_eq!(
            outcome.next_classification,
            RemoteClassificationV1::Cancelled
        );
        assert!(!outcome.creates_intent);
        assert!(!outcome.creates_attempt);
        assert!(!outcome.creates_run);
        assert!(!outcome.performs_provider_io);
        assert!(!outcome.refunds_or_remints);
    }
}
