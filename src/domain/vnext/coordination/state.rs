use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::vnext::authority::{PrincipalIdV1, SessionIdV1};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::model::{
    AudienceMemberV1, ConflictIdV1, CoordinationErrorV1, CoordinationMessageContentV1,
    CoordinationSubjectRefV1, ExactMessageRefV1, FocusIdV1, FocusSubjectV1, MessageIdV1,
    RepositoryInstallationRefV1, ScopeAtomV1, ScopeIdV1, StoreOrderV1, ThreadIdV1,
    TrustedIntervalV1, WithdrawalReasonV1, actor_value, bytes, hash_value, optional_message_ref,
    validate_audience,
};

const MAX_MESSAGES_PER_THREAD_V1: usize = 1_000_000;
const MAX_SCOPE_ATOMS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ThreadDescriptorV1 {
    pub(crate) thread_id: ThreadIdV1,
    pub(crate) revision: u64,
    pub(crate) audience: Vec<AudienceMemberV1>,
    pub(crate) audience_hash: [u8; 32],
}

impl ThreadDescriptorV1 {
    pub(crate) fn new(
        thread_id: ThreadIdV1,
        audience: Vec<AudienceMemberV1>,
    ) -> Result<Self, CoordinationStateErrorV1> {
        let audience_hash = validate_audience(&audience)?;
        Ok(Self {
            thread_id,
            revision: 1,
            audience,
            audience_hash,
        })
    }

    pub(crate) fn admits(&self, principal: PrincipalIdV1) -> bool {
        self.audience
            .iter()
            .any(|member| member.snapshot().admits(principal))
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.thread_id.as_bytes()),
            CborValue::Unsigned(self.revision),
            CborValue::Array(
                self.audience
                    .iter()
                    .map(AudienceMemberV1::canonical_value)
                    .collect(),
            ),
            bytes(&self.audience_hash),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MessageV1 {
    pub(crate) message_id: MessageIdV1,
    pub(crate) semantic_hash: [u8; 32],
    pub(crate) content_hash: [u8; 32],
    pub(crate) store_order: StoreOrderV1,
    pub(crate) thread_id: ThreadIdV1,
    pub(crate) thread_revision: u64,
    pub(crate) audience_hash: [u8; 32],
    pub(crate) author_principal: PrincipalIdV1,
    pub(crate) author_session: SessionIdV1,
    pub(crate) content: CoordinationMessageContentV1,
    pub(crate) subject_refs: Vec<CoordinationSubjectRefV1>,
    pub(crate) reply_to: Option<ExactMessageRefV1>,
    pub(crate) correction_of: Option<ExactMessageRefV1>,
    pub(crate) issued_at: u64,
}

impl MessageV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "MessageV1 closes the exact immutable Coordination record"
    )]
    pub(crate) fn new(
        message_id: MessageIdV1,
        store_order: StoreOrderV1,
        thread_id: ThreadIdV1,
        thread_revision: u64,
        audience_hash: [u8; 32],
        author_principal: PrincipalIdV1,
        author_session: SessionIdV1,
        content: CoordinationMessageContentV1,
        subject_refs: Vec<CoordinationSubjectRefV1>,
        reply_to: Option<ExactMessageRefV1>,
        correction_of: Option<ExactMessageRefV1>,
        issued_at: u64,
    ) -> Result<Self, CoordinationStateErrorV1> {
        content.validate()?;
        if thread_revision == 0
            || audience_hash == [0; 32]
            || subject_refs.len() > super::model::MAX_SUBJECT_REFS_V1
            || subject_refs.windows(2).any(|pair| pair[0] >= pair[1])
            || issued_at == 0
        {
            return Err(CoordinationStateErrorV1::InvalidMessage);
        }
        subject_refs.iter().try_for_each(|value| value.validate())?;
        let subject_values = subject_refs
            .iter()
            .map(CoordinationSubjectRefV1::canonical_value)
            .collect();
        let content_hash = hash_value(
            "maestro.vnext.coordination-message-content.v1",
            &content.canonical_value(),
        )?;
        let semantic_hash = hash_value(
            "maestro.vnext.coordination-message.v1",
            &CborValue::Array(vec![
                bytes(message_id.as_bytes()),
                store_order.canonical_value(),
                bytes(thread_id.as_bytes()),
                CborValue::Unsigned(thread_revision),
                bytes(&audience_hash),
                actor_value(author_principal, author_session),
                bytes(&content_hash),
                CborValue::Array(subject_values),
                optional_message_ref(reply_to.as_ref()),
                optional_message_ref(correction_of.as_ref()),
                CborValue::Unsigned(issued_at),
            ]),
        )?;
        Ok(Self {
            message_id,
            semantic_hash,
            content_hash,
            store_order,
            thread_id,
            thread_revision,
            audience_hash,
            author_principal,
            author_session,
            content,
            subject_refs,
            reply_to,
            correction_of,
            issued_at,
        })
    }

    pub(crate) fn exact_ref(&self) -> Result<ExactMessageRefV1, CoordinationErrorV1> {
        ExactMessageRefV1::new(self.message_id, self.semantic_hash)
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.message_id.as_bytes()),
            bytes(&self.semantic_hash),
            bytes(&self.content_hash),
            self.store_order.canonical_value(),
            bytes(self.thread_id.as_bytes()),
            CborValue::Unsigned(self.thread_revision),
            bytes(&self.audience_hash),
            actor_value(self.author_principal, self.author_session),
            self.content.canonical_value(),
            CborValue::Array(
                self.subject_refs
                    .iter()
                    .map(CoordinationSubjectRefV1::canonical_value)
                    .collect(),
            ),
            optional_message_ref(self.reply_to.as_ref()),
            optional_message_ref(self.correction_of.as_ref()),
            CborValue::Unsigned(self.issued_at),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MessageAcknowledgementV1 {
    pub(crate) message_ref: ExactMessageRefV1,
    pub(crate) acknowledging_principal: PrincipalIdV1,
    pub(crate) actor_session: SessionIdV1,
    pub(crate) via_address: super::model::CoordinationAddressV1,
    pub(crate) audience_hash: [u8; 32],
    pub(crate) eligibility_snapshot_id: [u8; 32],
    pub(crate) eligibility_snapshot_hash: [u8; 32],
    pub(crate) acknowledged_at: u64,
}

impl MessageAcknowledgementV1 {
    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.message_ref.canonical_value(),
            bytes(self.acknowledging_principal.as_bytes()),
            bytes(self.actor_session.as_bytes()),
            self.via_address.canonical_value(),
            bytes(&self.audience_hash),
            bytes(&self.eligibility_snapshot_id),
            bytes(&self.eligibility_snapshot_hash),
            CborValue::Unsigned(self.acknowledged_at),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FocusDeclarationV1 {
    pub(crate) focus_id: FocusIdV1,
    pub(crate) repository_installation: RepositoryInstallationRefV1,
    pub(crate) principal: PrincipalIdV1,
    pub(crate) session: SessionIdV1,
    pub(crate) subject: FocusSubjectV1,
    pub(crate) validity: TrustedIntervalV1,
    pub(crate) published_order: StoreOrderV1,
}

impl FocusDeclarationV1 {
    pub(crate) fn new(
        focus_id: FocusIdV1,
        repository_installation: RepositoryInstallationRefV1,
        principal: PrincipalIdV1,
        session: SessionIdV1,
        subject: FocusSubjectV1,
        validity: TrustedIntervalV1,
        published_order: StoreOrderV1,
    ) -> Result<Self, CoordinationStateErrorV1> {
        subject.validate()?;
        Ok(Self {
            focus_id,
            repository_installation,
            principal,
            session,
            subject,
            validity,
            published_order,
        })
    }

    fn actor_key(&self) -> ActorKeyV1 {
        ActorKeyV1 {
            repository_installation: self.repository_installation.clone(),
            principal: self.principal,
            session: self.session,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.focus_id.as_bytes()),
            super::model::text(self.repository_installation.as_str()),
            actor_value(self.principal, self.session),
            self.subject.canonical_value(),
            self.validity.canonical_value(),
            self.published_order.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScopeDeclarationV1 {
    pub(crate) scope_id: ScopeIdV1,
    pub(crate) repository_installation: RepositoryInstallationRefV1,
    pub(crate) principal: PrincipalIdV1,
    pub(crate) session: SessionIdV1,
    pub(crate) atoms: Vec<ScopeAtomV1>,
    pub(crate) validity: TrustedIntervalV1,
    pub(crate) published_order: StoreOrderV1,
}

impl ScopeDeclarationV1 {
    pub(crate) fn new(
        scope_id: ScopeIdV1,
        repository_installation: RepositoryInstallationRefV1,
        principal: PrincipalIdV1,
        session: SessionIdV1,
        atoms: Vec<ScopeAtomV1>,
        validity: TrustedIntervalV1,
        published_order: StoreOrderV1,
    ) -> Result<Self, CoordinationStateErrorV1> {
        if atoms.is_empty()
            || atoms.len() > MAX_SCOPE_ATOMS_V1
            || atoms.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CoordinationStateErrorV1::InvalidScope);
        }
        Ok(Self {
            scope_id,
            repository_installation,
            principal,
            session,
            atoms,
            validity,
            published_order,
        })
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.scope_id.as_bytes()),
            super::model::text(self.repository_installation.as_str()),
            actor_value(self.principal, self.session),
            CborValue::Array(
                self.atoms
                    .iter()
                    .map(ScopeAtomV1::canonical_value)
                    .collect(),
            ),
            self.validity.canonical_value(),
            self.published_order.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeclarationWithdrawalV1<I> {
    pub(crate) subject_id: I,
    pub(crate) principal: PrincipalIdV1,
    pub(crate) session: SessionIdV1,
    pub(crate) reason: WithdrawalReasonV1,
    pub(crate) withdrawn_at: u64,
    pub(crate) store_order: StoreOrderV1,
}

impl DeclarationWithdrawalV1<FocusIdV1> {
    fn canonical_focus_value(&self) -> CborValue {
        withdrawal_value(
            self.subject_id.as_bytes(),
            self.principal,
            self.session,
            self.reason,
            self.withdrawn_at,
            self.store_order,
        )
    }
}

impl DeclarationWithdrawalV1<ScopeIdV1> {
    fn canonical_scope_value(&self) -> CborValue {
        withdrawal_value(
            self.subject_id.as_bytes(),
            self.principal,
            self.session,
            self.reason,
            self.withdrawn_at,
            self.store_order,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoordinationRecordV1 {
    Thread(ThreadDescriptorV1),
    Message(Box<MessageV1>),
    Acknowledgement(MessageAcknowledgementV1),
    Focus(FocusDeclarationV1),
    FocusWithdrawal(DeclarationWithdrawalV1<FocusIdV1>),
    Scope(ScopeDeclarationV1),
    ScopeWithdrawal(DeclarationWithdrawalV1<ScopeIdV1>),
}

impl CoordinationRecordV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Thread(value) => tagged(1, value.canonical_value()),
            Self::Message(value) => tagged(2, value.canonical_value()),
            Self::Acknowledgement(value) => tagged(3, value.canonical_value()),
            Self::Focus(value) => tagged(4, value.canonical_value()),
            Self::FocusWithdrawal(value) => tagged(5, value.canonical_focus_value()),
            Self::Scope(value) => tagged(6, value.canonical_value()),
            Self::ScopeWithdrawal(value) => tagged(7, value.canonical_scope_value()),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ActorKeyV1 {
    repository_installation: RepositoryInstallationRefV1,
    principal: PrincipalIdV1,
    session: SessionIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ThreadStateV1 {
    descriptor: ThreadDescriptorV1,
    current_revision: u64,
    message_ids: Vec<MessageIdV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConflictStateV1 {
    assert_ref: ExactMessageRefV1,
    resolve_ref: Option<ExactMessageRefV1>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CoordinationStateV1 {
    threads: BTreeMap<ThreadIdV1, ThreadStateV1>,
    messages: BTreeMap<MessageIdV1, MessageV1>,
    acknowledgements: BTreeMap<(MessageIdV1, PrincipalIdV1), MessageAcknowledgementV1>,
    focuses: BTreeMap<FocusIdV1, FocusDeclarationV1>,
    current_focus: BTreeMap<ActorKeyV1, FocusIdV1>,
    focus_withdrawals: BTreeMap<FocusIdV1, DeclarationWithdrawalV1<FocusIdV1>>,
    scopes: BTreeMap<ScopeIdV1, ScopeDeclarationV1>,
    scope_withdrawals: BTreeMap<ScopeIdV1, DeclarationWithdrawalV1<ScopeIdV1>>,
    conflicts: BTreeMap<ConflictIdV1, ConflictStateV1>,
}

impl CoordinationStateV1 {
    pub(crate) fn message(&self, id: MessageIdV1) -> Option<&MessageV1> {
        self.messages.get(&id)
    }

    pub(crate) fn thread(&self, id: ThreadIdV1) -> Option<&ThreadDescriptorV1> {
        self.threads.get(&id).map(|thread| &thread.descriptor)
    }

    pub(crate) fn messages_in_total_order(&self) -> Vec<&MessageV1> {
        let mut messages = self.messages.values().collect::<Vec<_>>();
        messages.sort_by_key(|message| message.store_order);
        messages
    }

    pub(crate) fn acknowledgement(
        &self,
        message: MessageIdV1,
        principal: PrincipalIdV1,
    ) -> Option<&MessageAcknowledgementV1> {
        self.acknowledgements.get(&(message, principal))
    }

    pub(crate) fn current_focus(
        &self,
        repository: &RepositoryInstallationRefV1,
        principal: PrincipalIdV1,
        session: SessionIdV1,
    ) -> Option<&FocusDeclarationV1> {
        let key = ActorKeyV1 {
            repository_installation: repository.clone(),
            principal,
            session,
        };
        self.current_focus
            .get(&key)
            .and_then(|focus| self.focuses.get(focus))
    }

    pub(crate) fn applicable_focuses(&self, as_of: u64) -> Vec<&FocusDeclarationV1> {
        self.current_focus
            .values()
            .filter_map(|focus| self.focuses.get(focus))
            .filter(|focus| {
                focus.validity.contains(as_of)
                    && !self.focus_withdrawals.contains_key(&focus.focus_id)
            })
            .collect()
    }

    pub(crate) fn applicable_scopes(&self, as_of: u64) -> Vec<&ScopeDeclarationV1> {
        self.scopes
            .values()
            .filter(|scope| {
                scope.validity.contains(as_of)
                    && !self.scope_withdrawals.contains_key(&scope.scope_id)
            })
            .collect()
    }

    pub(crate) fn conflict_refs(
        &self,
        conflict_id: ConflictIdV1,
    ) -> Option<(&ExactMessageRefV1, Option<&ExactMessageRefV1>)> {
        self.conflicts
            .get(&conflict_id)
            .map(|conflict| (&conflict.assert_ref, conflict.resolve_ref.as_ref()))
    }

    pub(crate) fn semantic_hash(&self) -> Result<[u8; 32], CoordinationStateErrorV1> {
        let mut records = Vec::new();
        records.extend(
            self.threads
                .values()
                .map(|thread| tagged(1, thread.descriptor.canonical_value())),
        );
        records.extend(
            self.messages
                .values()
                .map(|message| tagged(2, message.canonical_value())),
        );
        records.extend(
            self.acknowledgements
                .values()
                .map(|ack| tagged(3, ack.canonical_value())),
        );
        records.extend(
            self.focuses
                .values()
                .map(|focus| tagged(4, focus.canonical_value())),
        );
        records.extend(
            self.focus_withdrawals
                .values()
                .map(|withdrawal| tagged(5, withdrawal.canonical_focus_value())),
        );
        records.extend(
            self.scopes
                .values()
                .map(|scope| tagged(6, scope.canonical_value())),
        );
        records.extend(
            self.scope_withdrawals
                .values()
                .map(|withdrawal| tagged(7, withdrawal.canonical_scope_value())),
        );
        Ok(hash_value(
            "maestro.vnext.coordination-state.v1",
            &CborValue::Array(records),
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoordinationMutationV1 {
    PublishInitialMessage {
        thread: ThreadDescriptorV1,
        message: MessageV1,
    },
    PublishMessage {
        expected_thread_revision: u64,
        message: MessageV1,
    },
    AcknowledgeMessage {
        acknowledgement: MessageAcknowledgementV1,
    },
    ReplaceFocus {
        expected_current: Option<FocusIdV1>,
        replacement: FocusDeclarationV1,
        withdrawal_order: Option<StoreOrderV1>,
        withdrawn_at: Option<u64>,
    },
    WithdrawFocus {
        focus_id: FocusIdV1,
        principal: PrincipalIdV1,
        session: SessionIdV1,
        withdrawn_at: u64,
        store_order: StoreOrderV1,
    },
    PublishScope {
        scope: ScopeDeclarationV1,
    },
    WithdrawScope {
        scope_id: ScopeIdV1,
        principal: PrincipalIdV1,
        session: SessionIdV1,
        withdrawn_at: u64,
        store_order: StoreOrderV1,
    },
    AssertConflict {
        expected_thread_revision: u64,
        message: MessageV1,
    },
    ResolveConflict {
        expected_thread_revision: u64,
        message: MessageV1,
    },
}

impl CoordinationMutationV1 {
    pub(crate) const fn action_literal(&self) -> &'static str {
        match self {
            Self::PublishInitialMessage { .. } => "PublishInitialMessage",
            Self::PublishMessage { .. } => "PublishMessage",
            Self::AcknowledgeMessage { .. } => "AcknowledgeMessage",
            Self::ReplaceFocus { .. } => "ReplaceFocus",
            Self::WithdrawFocus { .. } => "WithdrawFocus",
            Self::PublishScope { .. } => "PublishScope",
            Self::WithdrawScope { .. } => "WithdrawScope",
            Self::AssertConflict { .. } => "AssertConflict",
            Self::ResolveConflict { .. } => "ResolveConflict",
        }
    }

    pub(crate) fn actor(&self) -> (PrincipalIdV1, SessionIdV1) {
        match self {
            Self::PublishInitialMessage { message, .. }
            | Self::PublishMessage { message, .. }
            | Self::AssertConflict { message, .. }
            | Self::ResolveConflict { message, .. } => {
                (message.author_principal, message.author_session)
            }
            Self::AcknowledgeMessage { acknowledgement } => (
                acknowledgement.acknowledging_principal,
                acknowledgement.actor_session,
            ),
            Self::ReplaceFocus { replacement, .. } => (replacement.principal, replacement.session),
            Self::WithdrawFocus {
                principal, session, ..
            }
            | Self::WithdrawScope {
                principal, session, ..
            } => (*principal, *session),
            Self::PublishScope { scope } => (scope.principal, scope.session),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CoordinationTransitionV1 {
    action_literal: &'static str,
    subject_commitment: [u8; 32],
    owner_basis_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    actor_principal: PrincipalIdV1,
    actor_session: SessionIdV1,
    records: Vec<CoordinationRecordV1>,
}

impl CoordinationTransitionV1 {
    pub(crate) const fn action_literal(&self) -> &'static str {
        self.action_literal
    }

    pub(crate) const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub(crate) const fn owner_basis_commitment(&self) -> [u8; 32] {
        self.owner_basis_commitment
    }

    pub(crate) const fn payload_commitment(&self) -> [u8; 32] {
        self.payload_commitment
    }

    pub(crate) const fn actor_principal(&self) -> PrincipalIdV1 {
        self.actor_principal
    }

    pub(crate) const fn actor_session(&self) -> SessionIdV1 {
        self.actor_session
    }

    pub(crate) fn records(&self) -> &[CoordinationRecordV1] {
        &self.records
    }
}

pub(crate) fn apply_coordination_mutation(
    state: &mut CoordinationStateV1,
    mutation: CoordinationMutationV1,
) -> Result<CoordinationTransitionV1, CoordinationStateErrorV1> {
    let action_literal = mutation.action_literal();
    let actor = mutation.actor();
    let (subject, owner_basis, records) = match &mutation {
        CoordinationMutationV1::PublishInitialMessage { thread, message } => {
            publish_initial_message(state, thread, message)?
        }
        CoordinationMutationV1::PublishMessage {
            expected_thread_revision,
            message,
        } => publish_message(state, *expected_thread_revision, message, None)?,
        CoordinationMutationV1::AcknowledgeMessage { acknowledgement } => {
            acknowledge_message(state, acknowledgement)?
        }
        CoordinationMutationV1::ReplaceFocus {
            expected_current,
            replacement,
            withdrawal_order,
            withdrawn_at,
        } => replace_focus(
            state,
            *expected_current,
            replacement,
            *withdrawal_order,
            *withdrawn_at,
        )?,
        CoordinationMutationV1::WithdrawFocus {
            focus_id,
            principal,
            session,
            withdrawn_at,
            store_order,
        } => withdraw_focus(
            state,
            *focus_id,
            *principal,
            *session,
            *withdrawn_at,
            *store_order,
        )?,
        CoordinationMutationV1::PublishScope { scope } => publish_scope(state, scope)?,
        CoordinationMutationV1::WithdrawScope {
            scope_id,
            principal,
            session,
            withdrawn_at,
            store_order,
        } => withdraw_scope(
            state,
            *scope_id,
            *principal,
            *session,
            *withdrawn_at,
            *store_order,
        )?,
        CoordinationMutationV1::AssertConflict {
            expected_thread_revision,
            message,
        } => assert_conflict(state, *expected_thread_revision, message)?,
        CoordinationMutationV1::ResolveConflict {
            expected_thread_revision,
            message,
        } => resolve_conflict(state, *expected_thread_revision, message)?,
    };
    let payload = mutation_value(&mutation);
    let payload_commitment =
        hash_value("maestro.vnext.coordination-mutation-payload.v1", &payload)?;
    Ok(CoordinationTransitionV1 {
        action_literal,
        subject_commitment: hash_value("maestro.vnext.coordination-mutation-subject.v1", &subject)?,
        owner_basis_commitment: hash_value(
            "maestro.vnext.coordination-owner-basis.v1",
            &owner_basis,
        )?,
        payload_commitment,
        actor_principal: actor.0,
        actor_session: actor.1,
        records,
    })
}

fn publish_initial_message(
    state: &mut CoordinationStateV1,
    thread: &ThreadDescriptorV1,
    message: &MessageV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    if state.threads.contains_key(&thread.thread_id)
        || state.messages.contains_key(&message.message_id)
        || state
            .messages
            .values()
            .any(|existing| existing.store_order == message.store_order)
        || message.thread_id != thread.thread_id
        || message.thread_revision != thread.revision
        || message.audience_hash != thread.audience_hash
        || !thread.admits(message.author_principal)
    {
        return Err(CoordinationStateErrorV1::InitialPublicationConflict);
    }
    validate_message_refs(state, thread, message)?;
    if matches!(
        &message.content,
        CoordinationMessageContentV1::ConflictResolve { .. }
    ) {
        return Err(CoordinationStateErrorV1::InvalidConflict);
    }
    let mut conflicts = state.conflicts.clone();
    register_assert(&mut conflicts, message)?;
    state.threads.insert(
        thread.thread_id,
        ThreadStateV1 {
            descriptor: thread.clone(),
            current_revision: thread.revision,
            message_ids: vec![message.message_id],
        },
    );
    state.messages.insert(message.message_id, message.clone());
    state.conflicts = conflicts;
    Ok((
        bytes(thread.thread_id.as_bytes()),
        CborValue::Array(vec![CborValue::Unsigned(0)]),
        vec![
            CoordinationRecordV1::Thread(thread.clone()),
            CoordinationRecordV1::Message(Box::new(message.clone())),
        ],
    ))
}

fn publish_message(
    state: &mut CoordinationStateV1,
    expected_thread_revision: u64,
    message: &MessageV1,
    required_conflict_kind: Option<bool>,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    if state.messages.contains_key(&message.message_id) {
        return Err(CoordinationStateErrorV1::DuplicateMessage);
    }
    if state
        .messages
        .values()
        .any(|existing| existing.store_order == message.store_order)
    {
        return Err(CoordinationStateErrorV1::DuplicateMessageStoreOrder);
    }
    let current = state
        .threads
        .get(&message.thread_id)
        .ok_or(CoordinationStateErrorV1::UnknownThread)?;
    if current.current_revision != expected_thread_revision
        || message.thread_revision != expected_thread_revision + 1
        || message.audience_hash != current.descriptor.audience_hash
        || current.message_ids.len() >= MAX_MESSAGES_PER_THREAD_V1
        || !current.descriptor.admits(message.author_principal)
    {
        return Err(CoordinationStateErrorV1::StaleThread);
    }
    match required_conflict_kind {
        Some(false)
            if !matches!(
                &message.content,
                CoordinationMessageContentV1::ConflictAssert { .. }
            ) =>
        {
            return Err(CoordinationStateErrorV1::InvalidConflict);
        }
        Some(true)
            if !matches!(
                &message.content,
                CoordinationMessageContentV1::ConflictResolve { .. }
            ) =>
        {
            return Err(CoordinationStateErrorV1::InvalidConflict);
        }
        None if matches!(
            &message.content,
            CoordinationMessageContentV1::ConflictAssert { .. }
                | CoordinationMessageContentV1::ConflictResolve { .. }
        ) =>
        {
            return Err(CoordinationStateErrorV1::InvalidConflict);
        }
        _ => {}
    }
    validate_message_refs(state, &current.descriptor, message)?;
    let prior_basis = CborValue::Array(vec![
        bytes(message.thread_id.as_bytes()),
        CborValue::Unsigned(current.current_revision),
        bytes(&current.descriptor.audience_hash),
    ]);
    let mut conflicts = state.conflicts.clone();
    register_assert(&mut conflicts, message)?;
    register_resolution(&mut conflicts, message)?;
    let thread = state
        .threads
        .get_mut(&message.thread_id)
        .expect("invariant: Thread existence checked above");
    thread.current_revision += 1;
    thread.message_ids.push(message.message_id);
    state.messages.insert(message.message_id, message.clone());
    state.conflicts = conflicts;
    Ok((
        bytes(message.thread_id.as_bytes()),
        prior_basis,
        vec![CoordinationRecordV1::Message(Box::new(message.clone()))],
    ))
}

fn acknowledge_message(
    state: &mut CoordinationStateV1,
    acknowledgement: &MessageAcknowledgementV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    let message = state
        .messages
        .get(&acknowledgement.message_ref.message_id)
        .ok_or(CoordinationStateErrorV1::UnknownMessage)?;
    let thread = state
        .threads
        .get(&message.thread_id)
        .expect("invariant: every Message belongs to a Thread");
    let member = thread
        .descriptor
        .audience
        .iter()
        .find(|member| {
            member.snapshot().address() == &acknowledgement.via_address
                && member
                    .snapshot()
                    .admits(acknowledgement.acknowledging_principal)
        })
        .ok_or(CoordinationStateErrorV1::NotAudienceEligible)?;
    let key = (
        acknowledgement.message_ref.message_id,
        acknowledgement.acknowledging_principal,
    );
    if message.semantic_hash != acknowledgement.message_ref.semantic_hash
        || message.audience_hash != acknowledgement.audience_hash
        || acknowledgement.eligibility_snapshot_id != *member.snapshot().id()
        || acknowledgement.eligibility_snapshot_hash != member.snapshot().semantic_hash()
        || acknowledgement.acknowledged_at == 0
        || state.acknowledgements.contains_key(&key)
    {
        return Err(CoordinationStateErrorV1::DuplicateOrInvalidAcknowledgement);
    }
    state.acknowledgements.insert(key, acknowledgement.clone());
    Ok((
        CborValue::Array(vec![
            bytes(acknowledgement.message_ref.message_id.as_bytes()),
            bytes(acknowledgement.acknowledging_principal.as_bytes()),
        ]),
        CborValue::Array(vec![CborValue::Unsigned(0)]),
        vec![CoordinationRecordV1::Acknowledgement(
            acknowledgement.clone(),
        )],
    ))
}

fn replace_focus(
    state: &mut CoordinationStateV1,
    expected_current: Option<FocusIdV1>,
    replacement: &FocusDeclarationV1,
    withdrawal_order: Option<StoreOrderV1>,
    withdrawn_at: Option<u64>,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    if state.focuses.contains_key(&replacement.focus_id) {
        return Err(CoordinationStateErrorV1::DuplicateFocus);
    }
    let key = replacement.actor_key();
    let actual = state.current_focus.get(&key).copied();
    if actual != expected_current {
        return Err(CoordinationStateErrorV1::StaleFocus);
    }
    let mut records = Vec::new();
    if let Some(current_id) = actual {
        let (Some(store_order), Some(withdrawn_at)) = (withdrawal_order, withdrawn_at) else {
            return Err(CoordinationStateErrorV1::MissingReplacementWithdrawal);
        };
        if state.focus_withdrawals.contains_key(&current_id) {
            return Err(CoordinationStateErrorV1::StaleFocus);
        }
        if withdrawn_at == 0 {
            return Err(CoordinationStateErrorV1::InvalidWithdrawal);
        }
        let withdrawal = DeclarationWithdrawalV1 {
            subject_id: current_id,
            principal: replacement.principal,
            session: replacement.session,
            reason: WithdrawalReasonV1::Replaced,
            withdrawn_at,
            store_order,
        };
        state
            .focus_withdrawals
            .insert(current_id, withdrawal.clone());
        records.push(CoordinationRecordV1::FocusWithdrawal(withdrawal));
    } else if withdrawal_order.is_some() || withdrawn_at.is_some() {
        return Err(CoordinationStateErrorV1::UnexpectedReplacementWithdrawal);
    }
    state
        .focuses
        .insert(replacement.focus_id, replacement.clone());
    state.current_focus.insert(key, replacement.focus_id);
    records.push(CoordinationRecordV1::Focus(replacement.clone()));
    Ok((
        actor_value(replacement.principal, replacement.session),
        optional_identity(expected_current.map(|id| *id.as_bytes())),
        records,
    ))
}

fn withdraw_focus(
    state: &mut CoordinationStateV1,
    focus_id: FocusIdV1,
    principal: PrincipalIdV1,
    session: SessionIdV1,
    withdrawn_at: u64,
    store_order: StoreOrderV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    let focus = state
        .focuses
        .get(&focus_id)
        .ok_or(CoordinationStateErrorV1::UnknownFocus)?;
    if focus.principal != principal
        || focus.session != session
        || withdrawn_at == 0
        || state.focus_withdrawals.contains_key(&focus_id)
        || state.current_focus.get(&focus.actor_key()) != Some(&focus_id)
    {
        return Err(CoordinationStateErrorV1::StaleFocus);
    }
    let key = focus.actor_key();
    let withdrawal = DeclarationWithdrawalV1 {
        subject_id: focus_id,
        principal,
        session,
        reason: WithdrawalReasonV1::Explicit,
        withdrawn_at,
        store_order,
    };
    state.focus_withdrawals.insert(focus_id, withdrawal.clone());
    state.current_focus.remove(&key);
    Ok((
        bytes(focus_id.as_bytes()),
        bytes(focus_id.as_bytes()),
        vec![CoordinationRecordV1::FocusWithdrawal(withdrawal)],
    ))
}

fn publish_scope(
    state: &mut CoordinationStateV1,
    scope: &ScopeDeclarationV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    if state.scopes.contains_key(&scope.scope_id) {
        return Err(CoordinationStateErrorV1::DuplicateScope);
    }
    state.scopes.insert(scope.scope_id, scope.clone());
    Ok((
        bytes(scope.scope_id.as_bytes()),
        CborValue::Array(vec![CborValue::Unsigned(0)]),
        vec![CoordinationRecordV1::Scope(scope.clone())],
    ))
}

fn withdraw_scope(
    state: &mut CoordinationStateV1,
    scope_id: ScopeIdV1,
    principal: PrincipalIdV1,
    session: SessionIdV1,
    withdrawn_at: u64,
    store_order: StoreOrderV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    let scope = state
        .scopes
        .get(&scope_id)
        .ok_or(CoordinationStateErrorV1::UnknownScope)?;
    if scope.principal != principal
        || scope.session != session
        || withdrawn_at == 0
        || state.scope_withdrawals.contains_key(&scope_id)
    {
        return Err(CoordinationStateErrorV1::StaleScope);
    }
    let withdrawal = DeclarationWithdrawalV1 {
        subject_id: scope_id,
        principal,
        session,
        reason: WithdrawalReasonV1::Explicit,
        withdrawn_at,
        store_order,
    };
    state.scope_withdrawals.insert(scope_id, withdrawal.clone());
    Ok((
        bytes(scope_id.as_bytes()),
        bytes(scope_id.as_bytes()),
        vec![CoordinationRecordV1::ScopeWithdrawal(withdrawal)],
    ))
}

fn assert_conflict(
    state: &mut CoordinationStateV1,
    expected_thread_revision: u64,
    message: &MessageV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    let Some(conflict_id) = message.content.conflict_id() else {
        return Err(CoordinationStateErrorV1::InvalidConflict);
    };
    if state.conflicts.contains_key(&conflict_id) {
        return Err(CoordinationStateErrorV1::DuplicateConflict);
    }
    let (_, basis, records) =
        publish_message(state, expected_thread_revision, message, Some(false))?;
    Ok((bytes(conflict_id.as_bytes()), basis, records))
}

fn resolve_conflict(
    state: &mut CoordinationStateV1,
    expected_thread_revision: u64,
    message: &MessageV1,
) -> Result<(CborValue, CborValue, Vec<CoordinationRecordV1>), CoordinationStateErrorV1> {
    let Some(conflict_id) = message.content.conflict_id() else {
        return Err(CoordinationStateErrorV1::InvalidConflict);
    };
    let conflict = state
        .conflicts
        .get(&conflict_id)
        .ok_or(CoordinationStateErrorV1::UnknownConflict)?;
    if conflict.resolve_ref.is_some() {
        return Err(CoordinationStateErrorV1::AlreadyResolved);
    }
    let basis = CborValue::Array(vec![
        conflict.assert_ref.canonical_value(),
        CborValue::Array(vec![CborValue::Unsigned(0)]),
    ]);
    let (_, _, records) = publish_message(state, expected_thread_revision, message, Some(true))?;
    Ok((bytes(conflict_id.as_bytes()), basis, records))
}

fn validate_message_refs(
    state: &CoordinationStateV1,
    thread: &ThreadDescriptorV1,
    message: &MessageV1,
) -> Result<(), CoordinationStateErrorV1> {
    for reference in [&message.reply_to, &message.correction_of]
        .into_iter()
        .flatten()
    {
        let target = state
            .messages
            .get(&reference.message_id)
            .ok_or(CoordinationStateErrorV1::UnknownMessageReference)?;
        if target.semantic_hash != reference.semantic_hash
            || target.thread_id != thread.thread_id
            || target.audience_hash != thread.audience_hash
        {
            return Err(CoordinationStateErrorV1::CrossAudienceMessageReference);
        }
    }
    Ok(())
}

fn register_assert(
    conflicts: &mut BTreeMap<ConflictIdV1, ConflictStateV1>,
    message: &MessageV1,
) -> Result<(), CoordinationStateErrorV1> {
    if let CoordinationMessageContentV1::ConflictAssert { conflict_id, .. } = &message.content {
        if conflicts.contains_key(conflict_id) {
            return Err(CoordinationStateErrorV1::DuplicateConflict);
        }
        conflicts.insert(
            *conflict_id,
            ConflictStateV1 {
                assert_ref: message.exact_ref()?,
                resolve_ref: None,
            },
        );
    }
    Ok(())
}

fn register_resolution(
    conflicts: &mut BTreeMap<ConflictIdV1, ConflictStateV1>,
    message: &MessageV1,
) -> Result<(), CoordinationStateErrorV1> {
    if let CoordinationMessageContentV1::ConflictResolve {
        conflict_id,
        assert_ref,
        ..
    } = &message.content
    {
        let conflict = conflicts
            .get_mut(conflict_id)
            .ok_or(CoordinationStateErrorV1::UnknownConflict)?;
        if conflict.assert_ref != *assert_ref {
            return Err(CoordinationStateErrorV1::ConflictAssertMismatch);
        }
        if conflict.resolve_ref.is_some() {
            return Err(CoordinationStateErrorV1::AlreadyResolved);
        }
        conflict.resolve_ref = Some(message.exact_ref()?);
    }
    Ok(())
}

fn mutation_value(mutation: &CoordinationMutationV1) -> CborValue {
    match mutation {
        CoordinationMutationV1::PublishInitialMessage { thread, message } => {
            CborValue::Array(vec![
                CborValue::Unsigned(1),
                thread.canonical_value(),
                message.canonical_value(),
            ])
        }
        CoordinationMutationV1::PublishMessage {
            expected_thread_revision,
            message,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Unsigned(*expected_thread_revision),
            message.canonical_value(),
        ]),
        CoordinationMutationV1::AcknowledgeMessage { acknowledgement } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            acknowledgement.canonical_value(),
        ]),
        CoordinationMutationV1::ReplaceFocus {
            expected_current,
            replacement,
            withdrawal_order,
            withdrawn_at,
        } => CborValue::Array(vec![
            CborValue::Unsigned(4),
            optional_identity(expected_current.map(|id| *id.as_bytes())),
            replacement.canonical_value(),
            optional_store_order(*withdrawal_order),
            optional_u64(*withdrawn_at),
        ]),
        CoordinationMutationV1::WithdrawFocus {
            focus_id,
            principal,
            session,
            withdrawn_at,
            store_order,
        } => CborValue::Array(vec![
            CborValue::Unsigned(5),
            bytes(focus_id.as_bytes()),
            actor_value(*principal, *session),
            CborValue::Unsigned(*withdrawn_at),
            store_order.canonical_value(),
        ]),
        CoordinationMutationV1::PublishScope { scope } => {
            CborValue::Array(vec![CborValue::Unsigned(6), scope.canonical_value()])
        }
        CoordinationMutationV1::WithdrawScope {
            scope_id,
            principal,
            session,
            withdrawn_at,
            store_order,
        } => CborValue::Array(vec![
            CborValue::Unsigned(7),
            bytes(scope_id.as_bytes()),
            actor_value(*principal, *session),
            CborValue::Unsigned(*withdrawn_at),
            store_order.canonical_value(),
        ]),
        CoordinationMutationV1::AssertConflict {
            expected_thread_revision,
            message,
        } => CborValue::Array(vec![
            CborValue::Unsigned(8),
            CborValue::Unsigned(*expected_thread_revision),
            message.canonical_value(),
        ]),
        CoordinationMutationV1::ResolveConflict {
            expected_thread_revision,
            message,
        } => CborValue::Array(vec![
            CborValue::Unsigned(9),
            CborValue::Unsigned(*expected_thread_revision),
            message.canonical_value(),
        ]),
    }
}

fn withdrawal_value(
    subject_id: &[u8; 32],
    principal: PrincipalIdV1,
    session: SessionIdV1,
    reason: WithdrawalReasonV1,
    withdrawn_at: u64,
    store_order: StoreOrderV1,
) -> CborValue {
    CborValue::Array(vec![
        bytes(subject_id),
        actor_value(principal, session),
        CborValue::Unsigned(reason.tag()),
        CborValue::Unsigned(withdrawn_at),
        store_order.canonical_value(),
    ])
}

fn tagged(tag: u64, value: CborValue) -> CborValue {
    CborValue::Array(vec![CborValue::Unsigned(tag), value])
}

fn optional_identity(value: Option<[u8; 32]>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), bytes(&value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

fn optional_store_order(value: Option<StoreOrderV1>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), value.canonical_value()]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

fn optional_u64(value: Option<u64>) -> CborValue {
    match value {
        Some(value) => CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(value)]),
        None => CborValue::Array(vec![CborValue::Unsigned(0)]),
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CoordinationStateErrorV1 {
    #[error(transparent)]
    Model(#[from] CoordinationErrorV1),
    #[error("initial Thread and Message publication conflicts with existing identity or audience")]
    InitialPublicationConflict,
    #[error("Message identity already exists")]
    DuplicateMessage,
    #[error("Message store-order key already exists")]
    DuplicateMessageStoreOrder,
    #[error("Thread does not exist")]
    UnknownThread,
    #[error("Thread revision, audience, author eligibility, or bound is stale")]
    StaleThread,
    #[error("Message reply or correction target does not exist")]
    UnknownMessageReference,
    #[error("Message reply or correction crosses Thread or audience")]
    CrossAudienceMessageReference,
    #[error("Message is malformed")]
    InvalidMessage,
    #[error("Message does not exist")]
    UnknownMessage,
    #[error("acknowledging Principal is not eligible through the exact audience snapshot")]
    NotAudienceEligible,
    #[error("Message acknowledgement already exists or does not bind exact Message/audience bytes")]
    DuplicateOrInvalidAcknowledgement,
    #[error("Focus identity already exists")]
    DuplicateFocus,
    #[error("Focus expected-current CAS lost")]
    StaleFocus,
    #[error("Focus replacement must atomically withdraw the prior Focus")]
    MissingReplacementWithdrawal,
    #[error("Focus replacement without a current Focus cannot publish a Withdrawal")]
    UnexpectedReplacementWithdrawal,
    #[error("Coordination Withdrawal trusted time is missing")]
    InvalidWithdrawal,
    #[error("Focus does not exist")]
    UnknownFocus,
    #[error("Scope identity already exists")]
    DuplicateScope,
    #[error("Scope declaration is empty, oversized, duplicated, or not canonical")]
    InvalidScope,
    #[error("Scope does not exist")]
    UnknownScope,
    #[error("Scope Withdrawal lost expected-state or actor match")]
    StaleScope,
    #[error("Conflict payload is not the required closed Assert or Resolve variant")]
    InvalidConflict,
    #[error("Conflict identity already has one Assert")]
    DuplicateConflict,
    #[error("Conflict Assert does not exist")]
    UnknownConflict,
    #[error("Conflict Resolve does not bind the exact Assert id and hash")]
    ConflictAssertMismatch,
    #[error("Conflict already has one successful Resolve")]
    AlreadyResolved,
}

#[cfg(test)]
pub(crate) mod test_adapter {
    use super::*;

    pub(crate) fn apply(
        state: &CoordinationStateV1,
        mutation: CoordinationMutationV1,
    ) -> Result<(CoordinationStateV1, CoordinationTransitionV1), CoordinationStateErrorV1> {
        let mut next = state.clone();
        let transition = apply_coordination_mutation(&mut next, mutation)?;
        Ok((next, transition))
    }
}
