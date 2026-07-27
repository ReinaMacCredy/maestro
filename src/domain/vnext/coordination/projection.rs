use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::authority::{PrincipalIdV1, SessionIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::model::{
    ConflictIdV1, ExactMessageRefV1, MessageIdV1, RepositoryInstallationRefV1, ScopeIdV1,
    StoreOrderV1, ThreadIdV1, bytes, text,
};
use super::state::{CoordinationStateV1, MessageV1, ScopeDeclarationV1};

const MAX_INBOX_PAGE_V1: usize = 1_000;
const MAX_PRESENCE_SIGNALS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InboxQueryV1 {
    pub(crate) principal: PrincipalIdV1,
    pub(crate) exact_snapshot_hash: [u8; 32],
    pub(crate) authorized_threads: BTreeSet<ThreadIdV1>,
    pub(crate) as_of: u64,
    pub(crate) limit: usize,
    pub(crate) after: Option<InboxContinuationV1>,
}

impl InboxQueryV1 {
    fn query_hash(&self) -> Result<[u8; 32], CoordinationProjectionErrorV1> {
        Ok(domain_hash(
            "maestro.vnext.coordination-inbox-query.v1",
            &CborValue::Array(vec![
                bytes(self.principal.as_bytes()),
                bytes(&self.exact_snapshot_hash),
                CborValue::Array(
                    self.authorized_threads
                        .iter()
                        .map(|thread| bytes(thread.as_bytes()))
                        .collect(),
                ),
                CborValue::Unsigned(self.as_of),
                CborValue::Unsigned(self.limit as u64),
            ]),
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InboxContinuationV1 {
    query_hash: [u8; 32],
    exact_snapshot_hash: [u8; 32],
    after: StoreOrderV1,
}

impl InboxContinuationV1 {
    pub(crate) const fn after(&self) -> StoreOrderV1 {
        self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InboxRowV1 {
    pub(crate) message_id: MessageIdV1,
    pub(crate) message_hash: [u8; 32],
    pub(crate) thread_id: ThreadIdV1,
    pub(crate) store_order: StoreOrderV1,
    pub(crate) acknowledged: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InboxPageV1 {
    pub(crate) exact_snapshot_hash: [u8; 32],
    pub(crate) rows: Vec<InboxRowV1>,
    pub(crate) has_more: bool,
    pub(crate) continuation: Option<InboxContinuationV1>,
}

pub(crate) fn project_inbox(
    state: &CoordinationStateV1,
    query: &InboxQueryV1,
) -> Result<InboxPageV1, CoordinationProjectionErrorV1> {
    if query.exact_snapshot_hash == [0; 32]
        || query.as_of == 0
        || query.limit == 0
        || query.limit > MAX_INBOX_PAGE_V1
    {
        return Err(CoordinationProjectionErrorV1::InvalidInboxQuery);
    }
    let query_hash = query.query_hash()?;
    if query.after.as_ref().is_some_and(|token| {
        token.query_hash != query_hash || token.exact_snapshot_hash != query.exact_snapshot_hash
    }) {
        return Err(CoordinationProjectionErrorV1::StaleContinuation);
    }
    let after = query.after.as_ref().map(InboxContinuationV1::after);
    let mut visible = state
        .messages_in_total_order()
        .into_iter()
        .filter(|message| query.authorized_threads.contains(&message.thread_id))
        .filter(|message| message.issued_at <= query.as_of)
        .filter(|message| {
            state
                .thread(message.thread_id)
                .is_some_and(|thread| thread.admits(query.principal))
        })
        .filter(|message| after.is_none_or(|after| message.store_order > after))
        .collect::<Vec<_>>();
    let has_more = visible.len() > query.limit;
    visible.truncate(query.limit);
    let rows = visible
        .into_iter()
        .map(|message| inbox_row(state, query.principal, message))
        .collect::<Vec<_>>();
    let continuation = if has_more {
        rows.last().map(|row| InboxContinuationV1 {
            query_hash,
            exact_snapshot_hash: query.exact_snapshot_hash,
            after: row.store_order,
        })
    } else {
        None
    };
    Ok(InboxPageV1 {
        exact_snapshot_hash: query.exact_snapshot_hash,
        rows,
        has_more,
        continuation,
    })
}

fn inbox_row(
    state: &CoordinationStateV1,
    principal: PrincipalIdV1,
    message: &MessageV1,
) -> InboxRowV1 {
    InboxRowV1 {
        message_id: message.message_id,
        message_hash: message.semantic_hash,
        thread_id: message.thread_id,
        store_order: message.store_order,
        acknowledged: state
            .acknowledgement(message.message_id, principal)
            .is_some(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConflictViewV1 {
    pub(crate) conflict_id: ConflictIdV1,
    pub(crate) assert_ref: ExactMessageRefV1,
    pub(crate) resolve_ref: Option<ExactMessageRefV1>,
    pub(crate) current: bool,
}

pub(crate) fn conflict_view(
    state: &CoordinationStateV1,
    conflict_id: ConflictIdV1,
) -> Option<ConflictViewV1> {
    state
        .conflict_refs(conflict_id)
        .map(|(assert_ref, resolve_ref)| ConflictViewV1 {
            conflict_id,
            assert_ref: assert_ref.clone(),
            resolve_ref: resolve_ref.cloned(),
            current: resolve_ref.is_none(),
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PresenceDispositionV1 {
    RecentSignals,
    Missing,
    Stale,
    Conflicting,
    BoundedAway,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PresenceSignalKindV1 {
    AuthenticatedActivity,
    FocusDeclaration,
    ScopeDeclaration,
    ExecutionObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresenceSignalV1 {
    pub(crate) principal: PrincipalIdV1,
    pub(crate) session: SessionIdV1,
    pub(crate) kind: PresenceSignalKindV1,
    pub(crate) source_ref: String,
    pub(crate) observed_at: u64,
    pub(crate) invalidated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresenceConditionV1 {
    pub(crate) principal: PrincipalIdV1,
    pub(crate) session: SessionIdV1,
    pub(crate) exact_snapshot_hash: [u8; 32],
    pub(crate) as_of: u64,
    pub(crate) disposition: PresenceDispositionV1,
    pub(crate) considered_signal_refs: Vec<String>,
}

pub(crate) fn project_presence(
    principal: PrincipalIdV1,
    session: SessionIdV1,
    exact_snapshot_hash: [u8; 32],
    as_of: u64,
    maximum_age: u64,
    bounded_complete: bool,
    signals: &[PresenceSignalV1],
) -> Result<PresenceConditionV1, CoordinationProjectionErrorV1> {
    if exact_snapshot_hash == [0; 32]
        || as_of == 0
        || maximum_age == 0
        || signals.len() > MAX_PRESENCE_SIGNALS_V1
        || signals
            .iter()
            .any(|signal| signal.source_ref.is_empty() || signal.observed_at == 0)
    {
        return Err(CoordinationProjectionErrorV1::InvalidPresenceInput);
    }
    let mut matching = signals
        .iter()
        .filter(|signal| {
            signal.principal == principal
                && signal.session == session
                && signal.observed_at <= as_of
        })
        .collect::<Vec<_>>();
    matching.sort_by_key(|signal| (&signal.source_ref, signal.kind));
    if matching
        .windows(2)
        .any(|pair| pair[0].source_ref == pair[1].source_ref && pair[0] != pair[1])
    {
        return Ok(PresenceConditionV1 {
            principal,
            session,
            exact_snapshot_hash,
            as_of,
            disposition: PresenceDispositionV1::Conflicting,
            considered_signal_refs: matching
                .iter()
                .map(|signal| signal.source_ref.clone())
                .collect(),
        });
    }
    let disposition = if !bounded_complete {
        PresenceDispositionV1::BoundedAway
    } else if matching.is_empty() {
        PresenceDispositionV1::Missing
    } else if matching.iter().any(|signal| {
        !signal.invalidated
            && signal.observed_at <= as_of
            && as_of - signal.observed_at <= maximum_age
    }) {
        PresenceDispositionV1::RecentSignals
    } else {
        PresenceDispositionV1::Stale
    };
    Ok(PresenceConditionV1 {
        principal,
        session,
        exact_snapshot_hash,
        as_of,
        disposition,
        considered_signal_refs: matching
            .iter()
            .map(|signal| signal.source_ref.clone())
            .collect(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScopeOverlapV1 {
    pub(crate) left: ScopeIdV1,
    pub(crate) right: ScopeIdV1,
    pub(crate) overlapping_atom_pairs: usize,
}

pub(crate) fn project_scope_overlaps(
    state: &CoordinationStateV1,
    repository: &RepositoryInstallationRefV1,
    as_of: u64,
) -> Vec<ScopeOverlapV1> {
    let scopes = state
        .applicable_scopes(as_of)
        .into_iter()
        .filter(|scope| &scope.repository_installation == repository)
        .collect::<Vec<_>>();
    let mut overlaps = Vec::new();
    for (index, left) in scopes.iter().enumerate() {
        for right in &scopes[index + 1..] {
            let count = overlap_count(left, right);
            if count > 0 {
                overlaps.push(ScopeOverlapV1 {
                    left: left.scope_id,
                    right: right.scope_id,
                    overlapping_atom_pairs: count,
                });
            }
        }
    }
    overlaps
}

fn overlap_count(left: &ScopeDeclarationV1, right: &ScopeDeclarationV1) -> usize {
    left.atoms
        .iter()
        .flat_map(|left| right.atoms.iter().map(move |right| (left, right)))
        .filter(|(left, right)| left.overlaps(right))
        .count()
}

fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            text(domain),
            value.clone(),
        ]))?)
        .into(),
    )
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CoordinationProjectionErrorV1 {
    #[error("Inbox query snapshot, trusted time, or bound is invalid")]
    InvalidInboxQuery,
    #[error("Inbox continuation is not bound to the exact query and snapshot")]
    StaleContinuation,
    #[error("Presence projection inputs are invalid or overflowed")]
    InvalidPresenceInput,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}
