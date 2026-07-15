use super::effect_home::HomeTokenV1;

/// Opaque identity for one immutable H2 control revision, Head, or writer term.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectIntentControlTokenV1(HomeTokenV1);

impl EffectIntentControlTokenV1 {
    pub const fn new(value: HomeTokenV1) -> Self {
        Self(value)
    }

    pub const fn as_home_token(&self) -> &HomeTokenV1 {
        &self.0
    }
}

/// The sole mutable selector for one home-local Effect Intent control product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlHeadV1 {
    pub intent: EffectIntentControlTokenV1,
    pub revision: EffectIntentControlTokenV1,
    pub writer_term: EffectIntentControlTokenV1,
}

/// An immutable joined snapshot selected only through a Control Head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlRevisionV1 {
    pub id: EffectIntentControlTokenV1,
}

/// A writer term has a distinct current-tenure role from transition contenders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlWriterTermKindV1 {
    Origination,
    SameHomeRestore,
}

impl EffectIntentControlWriterTermKindV1 {
    pub const ALL: [Self; 2] = [Self::Origination, Self::SameHomeRestore];
}

/// One immutable writer tenure. It cannot represent a transition contender.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlWriterTermV1 {
    pub id: EffectIntentControlTokenV1,
    pub kind: EffectIntentControlWriterTermKindV1,
}

/// Every exact contender that may propose a transition through the same Head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlTransitionContenderV1 {
    OriginalHandler,
    RecoveryCaller,
    PreSealLocalRejection,
    Seal,
    ResponseHandler,
    Terminalizer,
    Classifier,
    Reconciler,
    Redispatcher,
    Withdrawal,
    SameHomeRestoreWriter,
}

impl EffectIntentControlTransitionContenderV1 {
    pub const ALL: [Self; 11] = [
        Self::OriginalHandler,
        Self::RecoveryCaller,
        Self::PreSealLocalRejection,
        Self::Seal,
        Self::ResponseHandler,
        Self::Terminalizer,
        Self::Classifier,
        Self::Reconciler,
        Self::Redispatcher,
        Self::Withdrawal,
        Self::SameHomeRestoreWriter,
    ];
}

/// The only dispositions a concrete consumer census may report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlConsumerDispositionV1 {
    CandidateContractDefinition,
    CandidateProofReader,
    SealedV1AuditMigrationConsumer,
}

/// Candidate-only descriptor for roles and separately inventoried physical
/// sources. A classified source never becomes a writer or reader role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlReadWriteCohortDescriptorV1 {
    pub transition_contender_count: u8,
    pub writer_term_kind_count: u8,
    pub physical_semantic_consumer_count: u16,
    pub candidate_contract_definition_count: u16,
    pub candidate_proof_reader_count: u16,
    pub sealed_v1_audit_migration_consumer_count: u16,
    pub replacement_removal_target_count: u16,
    pub legacy_semantic_removal_consumer_count: u16,
    pub unresolved_actual_semantic_consumer_count: u16,
}

/// The only candidate mutation shape. Applying it is intentionally out of scope
/// for Stage 0; a future owner must perform the one carrier CAS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlTransitionV1 {
    pub contender: EffectIntentControlTransitionContenderV1,
    pub intent: EffectIntentControlTokenV1,
    pub expected_head: EffectIntentControlTokenV1,
    pub expected_revision: EffectIntentControlTokenV1,
    pub expected_writer_term: EffectIntentControlTokenV1,
    pub candidate_revision: EffectIntentControlTokenV1,
}
