use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::effect_home::HomeTokenV1;
use super::runtime::{EffectIntentIdV1, ExecutionActionV1, ExecutionAttemptOwnerV1};
use super::withdrawal::{EffectIntentLiveDispatchV1, RemoteClassificationV1};
use crate::domain::vnext::authority::ActionRequestIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectIntentControlTokenV1(HomeTokenV1);

impl EffectIntentControlTokenV1 {
    pub fn from_value(value: &CborValue) -> Result<Self, EffectIntentControlErrorV1> {
        let digest: [u8; 32] = Sha256::digest(deterministic_cbor::encode(value)?).into();
        if digest == [0; 32] {
            return Err(EffectIntentControlErrorV1::MissingCommitment);
        }
        Ok(Self(HomeTokenV1::new(digest)))
    }

    pub const fn new(value: HomeTokenV1) -> Self {
        Self(value)
    }

    pub const fn as_home_token(&self) -> &HomeTokenV1 {
        &self.0
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlWriterTermKindV1 {
    Origination,
    SameHomeRestore,
}

impl EffectIntentControlWriterTermKindV1 {
    pub const ALL: [Self; 2] = [Self::Origination, Self::SameHomeRestore];

    const fn tag(self) -> u64 {
        match self {
            Self::Origination => 1,
            Self::SameHomeRestore => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlWriterTermV1 {
    id: EffectIntentControlTokenV1,
    intent: EffectIntentIdV1,
    kind: EffectIntentControlWriterTermKindV1,
    home: HomeTokenV1,
    issuance_commitment: [u8; 32],
    prior_writer_term: Option<EffectIntentControlTokenV1>,
    fencing_receipt: Option<EffectIntentControlTokenV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SameHomeWriterFencingReceiptV1 {
    id: EffectIntentControlTokenV1,
    intent: EffectIntentIdV1,
    home: HomeTokenV1,
    prior_head: EffectIntentControlTokenV1,
    prior_writer_term: EffectIntentControlTokenV1,
    prior_store_head: [u8; 32],
    prior_store_generation: [u8; 32],
    fence_ordinal: u64,
    durable_store_fence_commitment: [u8; 32],
}

impl SameHomeWriterFencingReceiptV1 {
    pub(crate) fn issue(
        intent: EffectIntentIdV1,
        home: HomeTokenV1,
        prior_head: EffectIntentControlTokenV1,
        prior_writer_term: EffectIntentControlTokenV1,
        prior_store_head: [u8; 32],
        prior_store_generation: [u8; 32],
        fence_ordinal: u64,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        require_nonzero(prior_store_head)?;
        require_nonzero(prior_store_generation)?;
        if fence_ordinal == 0 {
            return Err(EffectIntentControlErrorV1::OldWriterNotFenced);
        }
        let fence_value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.store-issued-writer-fence.v1")?,
            bytes(intent.as_bytes()),
            bytes(home.as_bytes()),
            bytes(prior_head.as_bytes()),
            bytes(prior_writer_term.as_bytes()),
            bytes(&prior_store_head),
            bytes(&prior_store_generation),
            CborValue::Unsigned(fence_ordinal),
        ]);
        let durable_store_fence_commitment: [u8; 32] =
            Sha256::digest(deterministic_cbor::encode(&fence_value)?).into();
        require_nonzero(durable_store_fence_commitment)?;
        let id = EffectIntentControlTokenV1::from_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.same-home-writer-fencing-receipt.v1")?,
            bytes(intent.as_bytes()),
            bytes(home.as_bytes()),
            bytes(prior_head.as_bytes()),
            bytes(prior_writer_term.as_bytes()),
            bytes(&prior_store_head),
            bytes(&prior_store_generation),
            CborValue::Unsigned(fence_ordinal),
            bytes(&durable_store_fence_commitment),
        ]))?;
        Ok(Self {
            id,
            intent,
            home,
            prior_head,
            prior_writer_term,
            prior_store_head,
            prior_store_generation,
            fence_ordinal,
            durable_store_fence_commitment,
        })
    }

    pub const fn id(self) -> EffectIntentControlTokenV1 {
        self.id
    }

    pub const fn intent(self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn home(self) -> HomeTokenV1 {
        self.home
    }

    pub const fn prior_head(self) -> EffectIntentControlTokenV1 {
        self.prior_head
    }

    pub const fn prior_writer_term(self) -> EffectIntentControlTokenV1 {
        self.prior_writer_term
    }

    pub const fn durable_store_fence_commitment(self) -> [u8; 32] {
        self.durable_store_fence_commitment
    }

    pub const fn prior_store_head(self) -> [u8; 32] {
        self.prior_store_head
    }

    pub const fn prior_store_generation(self) -> [u8; 32] {
        self.prior_store_generation
    }

    pub const fn fence_ordinal(self) -> u64 {
        self.fence_ordinal
    }

    pub(crate) fn canonical_value(self) -> Result<CborValue, EffectIntentControlErrorV1> {
        Ok(CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.same-home-writer-fencing-receipt.v1")?,
                bytes(self.intent.as_bytes()),
                bytes(self.home.as_bytes()),
                bytes(self.prior_head.as_bytes()),
                bytes(self.prior_writer_term.as_bytes()),
                bytes(&self.prior_store_head),
                bytes(&self.prior_store_generation),
                CborValue::Unsigned(self.fence_ordinal),
                bytes(&self.durable_store_fence_commitment),
            ]),
        ]))
    }

    pub(crate) fn from_canonical_value(
        value: &CborValue,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [id, CborValue::Array(payload)] = fields.as_slice() else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [
            CborValue::Text(domain),
            intent,
            home,
            prior_head,
            prior_writer,
            prior_store_head,
            prior_store_generation,
            CborValue::Unsigned(fence_ordinal),
            fence,
        ] = payload.as_slice()
        else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        if domain != "maestro.vnext.same-home-writer-fencing-receipt.v1" {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        let rebuilt = Self::issue(
            EffectIntentIdV1::from_bytes(exact_control_digest(intent)?)
                .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)?,
            HomeTokenV1::new(exact_control_digest(home)?),
            EffectIntentControlTokenV1::new(HomeTokenV1::new(exact_control_digest(prior_head)?)),
            EffectIntentControlTokenV1::new(HomeTokenV1::new(exact_control_digest(prior_writer)?)),
            exact_control_digest(prior_store_head)?,
            exact_control_digest(prior_store_generation)?,
            *fence_ordinal,
        )?;
        if rebuilt.durable_store_fence_commitment() != exact_control_digest(fence)?
            || rebuilt.id().as_bytes() != &exact_control_digest(id)?
            || rebuilt.canonical_value()? != *value
        {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        Ok(rebuilt)
    }
}

impl EffectIntentControlWriterTermV1 {
    pub fn originate(
        intent: EffectIntentIdV1,
        home: HomeTokenV1,
        writer_commitment: [u8; 32],
    ) -> Result<Self, EffectIntentControlErrorV1> {
        require_nonzero(writer_commitment)?;
        let kind = EffectIntentControlWriterTermKindV1::Origination;
        let id = EffectIntentControlTokenV1::from_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-writer-term.v1")?,
            bytes(intent.as_bytes()),
            CborValue::Unsigned(kind.tag()),
            bytes(home.as_bytes()),
            bytes(&writer_commitment),
            CborValue::optional(None),
            CborValue::optional(None),
        ]))?;
        Ok(Self {
            id,
            intent,
            kind,
            home,
            issuance_commitment: writer_commitment,
            prior_writer_term: None,
            fencing_receipt: None,
        })
    }

    pub fn same_home_restore(
        receipt: SameHomeWriterFencingReceiptV1,
        continuity_commitment: [u8; 32],
    ) -> Result<Self, EffectIntentControlErrorV1> {
        require_nonzero(continuity_commitment)?;
        let kind = EffectIntentControlWriterTermKindV1::SameHomeRestore;
        let id = EffectIntentControlTokenV1::from_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-writer-term.v1")?,
            bytes(receipt.intent().as_bytes()),
            CborValue::Unsigned(kind.tag()),
            bytes(receipt.home().as_bytes()),
            bytes(&continuity_commitment),
            CborValue::optional(Some(bytes(receipt.prior_writer_term().as_bytes()))),
            CborValue::optional(Some(bytes(receipt.id().as_bytes()))),
        ]))?;
        Ok(Self {
            id,
            intent: receipt.intent(),
            kind,
            home: receipt.home(),
            issuance_commitment: continuity_commitment,
            prior_writer_term: Some(receipt.prior_writer_term()),
            fencing_receipt: Some(receipt.id()),
        })
    }

    pub const fn id(self) -> EffectIntentControlTokenV1 {
        self.id
    }

    pub const fn intent(self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn kind(self) -> EffectIntentControlWriterTermKindV1 {
        self.kind
    }

    pub const fn home(self) -> HomeTokenV1 {
        self.home
    }

    pub const fn prior_writer_term(self) -> Option<EffectIntentControlTokenV1> {
        self.prior_writer_term
    }

    pub const fn fencing_receipt(self) -> Option<EffectIntentControlTokenV1> {
        self.fencing_receipt
    }

    pub fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            bytes(self.intent.as_bytes()),
            CborValue::Unsigned(self.kind.tag()),
            bytes(self.home.as_bytes()),
            bytes(&self.issuance_commitment),
            CborValue::optional(self.prior_writer_term.map(|prior| bytes(prior.as_bytes()))),
            CborValue::optional(
                self.fencing_receipt
                    .map(|receipt| bytes(receipt.as_bytes())),
            ),
        ])
    }

    pub(crate) fn from_canonical_value(
        value: &CborValue,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [
            id,
            intent,
            CborValue::Unsigned(kind),
            home,
            issuance,
            prior,
            receipt,
        ] = fields.as_slice()
        else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let id = EffectIntentControlTokenV1::new(HomeTokenV1::new(exact_control_digest(id)?));
        let intent = EffectIntentIdV1::from_bytes(exact_control_digest(intent)?)
            .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)?;
        let home = HomeTokenV1::new(exact_control_digest(home)?);
        let issuance_commitment = exact_control_digest(issuance)?;
        require_nonzero(issuance_commitment)?;
        let kind = match kind {
            1 => EffectIntentControlWriterTermKindV1::Origination,
            2 => EffectIntentControlWriterTermKindV1::SameHomeRestore,
            _ => return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
        };
        let prior_writer_term = parse_optional_control_token(prior)?;
        let fencing_receipt = parse_optional_control_token(receipt)?;
        if matches!(kind, EffectIntentControlWriterTermKindV1::Origination)
            != (prior_writer_term.is_none() && fencing_receipt.is_none())
            || matches!(kind, EffectIntentControlWriterTermKindV1::SameHomeRestore)
                != (prior_writer_term.is_some() && fencing_receipt.is_some())
        {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        let expected = EffectIntentControlTokenV1::from_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-writer-term.v1")?,
            bytes(intent.as_bytes()),
            CborValue::Unsigned(kind.tag()),
            bytes(home.as_bytes()),
            bytes(&issuance_commitment),
            CborValue::optional(prior_writer_term.map(|prior| bytes(prior.as_bytes()))),
            CborValue::optional(fencing_receipt.map(|receipt| bytes(receipt.as_bytes()))),
        ]))?;
        let term = Self {
            id,
            intent,
            kind,
            home,
            issuance_commitment,
            prior_writer_term,
            fencing_receipt,
        };
        if id != expected || term.canonical_value() != *value {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        Ok(term)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlHealthV1 {
    Healthy,
    RecoveryRequired,
    IntegrityBlocked,
}

impl EffectIntentControlHealthV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Healthy => 1,
            Self::RecoveryRequired => 2,
            Self::IntegrityBlocked => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentControlRevisionV1 {
    id: EffectIntentControlTokenV1,
    intent: EffectIntentIdV1,
    attempt_history: Vec<ExecutionAttemptOwnerV1>,
    live_attempt: Option<ExecutionAttemptOwnerV1>,
    live_dispatch: EffectIntentLiveDispatchV1,
    classification: RemoteClassificationV1,
    dispatch_fence_high_water: u64,
    run_set_revision: u64,
    runs_closed: bool,
    material_commitment: [u8; 32],
    credential_commitment: [u8; 32],
    use_fence_commitment: [u8; 32],
    result_commitment: Option<[u8; 32]>,
    idempotency_commitment: Option<[u8; 32]>,
    health: EffectIntentControlHealthV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentControlRevisionPartsV1 {
    pub intent: EffectIntentIdV1,
    pub attempt_history: Vec<ExecutionAttemptOwnerV1>,
    pub live_attempt: Option<ExecutionAttemptOwnerV1>,
    pub live_dispatch: EffectIntentLiveDispatchV1,
    pub classification: RemoteClassificationV1,
    pub dispatch_fence_high_water: u64,
    pub run_set_revision: u64,
    pub runs_closed: bool,
    pub material_commitment: [u8; 32],
    pub credential_commitment: [u8; 32],
    pub use_fence_commitment: [u8; 32],
    pub result_commitment: Option<[u8; 32]>,
    pub idempotency_commitment: Option<[u8; 32]>,
    pub health: EffectIntentControlHealthV1,
}

impl EffectIntentControlRevisionV1 {
    pub fn new(
        parts: EffectIntentControlRevisionPartsV1,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        require_nonzero(parts.material_commitment)?;
        require_nonzero(parts.credential_commitment)?;
        require_nonzero(parts.use_fence_commitment)?;
        if parts.run_set_revision == 0
            || !legal_live_dispatch_classification(parts.live_dispatch, parts.classification)
            || parts.result_commitment.is_some() != parts.idempotency_commitment.is_some()
        {
            return Err(EffectIntentControlErrorV1::InvalidControlProduct);
        }
        for value in parts
            .result_commitment
            .into_iter()
            .chain(parts.idempotency_commitment)
        {
            require_nonzero(value)?;
        }
        let owners = parts
            .attempt_history
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if owners.len() != parts.attempt_history.len()
            || parts
                .attempt_history
                .iter()
                .any(|owner| matches!(owner, ExecutionAttemptOwnerV1::Step(_)))
            || parts
                .live_attempt
                .is_some_and(|live| !owners.contains(&live))
            || matches!(
                parts.live_dispatch,
                EffectIntentLiveDispatchV1::Reserved | EffectIntentLiveDispatchV1::Sealed
            ) && !matches!(
                parts.live_attempt,
                Some(ExecutionAttemptOwnerV1::Dispatch(_))
            )
            || parts.live_dispatch == EffectIntentLiveDispatchV1::None
                && parts.classification == RemoteClassificationV1::Dispatching
            || parts.live_dispatch == EffectIntentLiveDispatchV1::None
                && matches!(
                    parts.live_attempt,
                    Some(ExecutionAttemptOwnerV1::Dispatch(_))
                )
            || parts.dispatch_fence_high_water == 0
                && parts.live_dispatch != EffectIntentLiveDispatchV1::None
            || parts.live_attempt.is_none() != parts.runs_closed
        {
            return Err(EffectIntentControlErrorV1::InvalidAttemptClosure);
        }
        let value = control_revision_payload(&parts)?;
        Ok(Self {
            id: EffectIntentControlTokenV1::from_value(&value)?,
            intent: parts.intent,
            attempt_history: parts.attempt_history,
            live_attempt: parts.live_attempt,
            live_dispatch: parts.live_dispatch,
            classification: parts.classification,
            dispatch_fence_high_water: parts.dispatch_fence_high_water,
            run_set_revision: parts.run_set_revision,
            runs_closed: parts.runs_closed,
            material_commitment: parts.material_commitment,
            credential_commitment: parts.credential_commitment,
            use_fence_commitment: parts.use_fence_commitment,
            result_commitment: parts.result_commitment,
            idempotency_commitment: parts.idempotency_commitment,
            health: parts.health,
        })
    }

    pub const fn id(&self) -> EffectIntentControlTokenV1 {
        self.id
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub fn attempt_history(&self) -> &[ExecutionAttemptOwnerV1] {
        &self.attempt_history
    }

    pub const fn live_attempt(&self) -> Option<ExecutionAttemptOwnerV1> {
        self.live_attempt
    }

    pub const fn live_dispatch(&self) -> EffectIntentLiveDispatchV1 {
        self.live_dispatch
    }

    pub const fn classification(&self) -> RemoteClassificationV1 {
        self.classification
    }

    pub const fn dispatch_fence_high_water(&self) -> u64 {
        self.dispatch_fence_high_water
    }

    pub const fn run_set_revision(&self) -> u64 {
        self.run_set_revision
    }

    pub const fn runs_closed(&self) -> bool {
        self.runs_closed
    }

    pub const fn health(&self) -> EffectIntentControlHealthV1 {
        self.health
    }

    pub fn parts(&self) -> EffectIntentControlRevisionPartsV1 {
        EffectIntentControlRevisionPartsV1 {
            intent: self.intent,
            attempt_history: self.attempt_history.clone(),
            live_attempt: self.live_attempt,
            live_dispatch: self.live_dispatch,
            classification: self.classification,
            dispatch_fence_high_water: self.dispatch_fence_high_water,
            run_set_revision: self.run_set_revision,
            runs_closed: self.runs_closed,
            material_commitment: self.material_commitment,
            credential_commitment: self.credential_commitment,
            use_fence_commitment: self.use_fence_commitment,
            result_commitment: self.result_commitment,
            idempotency_commitment: self.idempotency_commitment,
            health: self.health,
        }
    }

    pub fn canonical_value(&self) -> Result<CborValue, EffectIntentControlErrorV1> {
        Ok(CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            control_revision_payload(&self.parts())?,
        ]))
    }

    pub(crate) fn from_canonical_value(
        value: &CborValue,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [stored_id, payload] = fields.as_slice() else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let CborValue::Array(parts) = payload else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [
            CborValue::Text(domain),
            intent,
            CborValue::Array(history),
            live_attempt,
            CborValue::Unsigned(live_dispatch),
            CborValue::Unsigned(classification),
            CborValue::Unsigned(dispatch_fence_high_water),
            CborValue::Unsigned(run_set_revision),
            CborValue::Bool(runs_closed),
            material,
            credential,
            use_fence,
            result,
            idempotency,
            CborValue::Unsigned(health),
        ] = parts.as_slice()
        else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        if domain != "maestro.vnext.effect-control-revision.v1" {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        let attempt_history = history
            .iter()
            .map(super::runtime::parse_attempt_owner)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)?;
        let live_attempt = parse_optional_control_owner(live_attempt)?;
        let rebuilt = Self::new(EffectIntentControlRevisionPartsV1 {
            intent: EffectIntentIdV1::from_bytes(exact_control_digest(intent)?)
                .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)?,
            attempt_history,
            live_attempt,
            live_dispatch: parse_live_dispatch(*live_dispatch)?,
            classification: parse_classification(*classification)?,
            dispatch_fence_high_water: *dispatch_fence_high_water,
            run_set_revision: *run_set_revision,
            runs_closed: *runs_closed,
            material_commitment: exact_control_digest(material)?,
            credential_commitment: exact_control_digest(credential)?,
            use_fence_commitment: exact_control_digest(use_fence)?,
            result_commitment: parse_optional_control_digest(result)?,
            idempotency_commitment: parse_optional_control_digest(idempotency)?,
            health: match health {
                1 => EffectIntentControlHealthV1::Healthy,
                2 => EffectIntentControlHealthV1::RecoveryRequired,
                3 => EffectIntentControlHealthV1::IntegrityBlocked,
                _ => return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
            },
        })?;
        if rebuilt.id().as_bytes() != &exact_control_digest(stored_id)?
            || rebuilt.canonical_value()? != *value
        {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentControlHeadV1 {
    id: EffectIntentControlTokenV1,
    intent: EffectIntentIdV1,
    home: HomeTokenV1,
    revision: EffectIntentControlTokenV1,
    writer_term: EffectIntentControlTokenV1,
}

impl EffectIntentControlHeadV1 {
    pub fn new(
        intent: EffectIntentIdV1,
        home: HomeTokenV1,
        revision: &EffectIntentControlRevisionV1,
        writer_term: EffectIntentControlWriterTermV1,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        if revision.intent() != intent
            || writer_term.intent() != intent
            || writer_term.home() != home
        {
            return Err(EffectIntentControlErrorV1::IntentMismatch);
        }
        let id = EffectIntentControlTokenV1::from_value(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-control-head.v1")?,
            bytes(intent.as_bytes()),
            bytes(home.as_bytes()),
            bytes(revision.id().as_bytes()),
            bytes(writer_term.id().as_bytes()),
        ]))?;
        Ok(Self {
            id,
            intent,
            home,
            revision: revision.id(),
            writer_term: writer_term.id(),
        })
    }

    pub const fn id(&self) -> EffectIntentControlTokenV1 {
        self.id
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn home(&self) -> HomeTokenV1 {
        self.home
    }

    pub const fn revision(&self) -> EffectIntentControlTokenV1 {
        self.revision
    }

    pub const fn writer_term(&self) -> EffectIntentControlTokenV1 {
        self.writer_term
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            bytes(self.intent.as_bytes()),
            bytes(self.home.as_bytes()),
            bytes(self.revision.as_bytes()),
            bytes(self.writer_term.as_bytes()),
        ])
    }

    pub(crate) fn from_canonical_value(
        value: &CborValue,
        revision: &EffectIntentControlRevisionV1,
        writer_term: EffectIntentControlWriterTermV1,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let [stored_id, intent, home, stored_revision, stored_writer] = fields.as_slice() else {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        };
        let intent = EffectIntentIdV1::from_bytes(exact_control_digest(intent)?)
            .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)?;
        let rebuilt = Self::new(
            intent,
            HomeTokenV1::new(exact_control_digest(home)?),
            revision,
            writer_term,
        )?;
        if rebuilt.id().as_bytes() != &exact_control_digest(stored_id)?
            || revision.id().as_bytes() != &exact_control_digest(stored_revision)?
            || writer_term.id().as_bytes() != &exact_control_digest(stored_writer)?
            || rebuilt.canonical_value() != *value
        {
            return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntentControlTransitionV1 {
    contender: EffectIntentControlTransitionContenderV1,
    intent: EffectIntentIdV1,
    expected_head: EffectIntentControlTokenV1,
    expected_revision: EffectIntentControlTokenV1,
    expected_writer_term: EffectIntentControlTokenV1,
    mutation: EffectIntentControlMutationV1,
    action_request_id: ActionRequestIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIntentControlPublicationCommitmentsV1 {
    result_commitment: [u8; 32],
    idempotency_commitment: [u8; 32],
}

impl EffectIntentControlPublicationCommitmentsV1 {
    pub(crate) fn from_store_publication(
        result_commitment: [u8; 32],
        idempotency_commitment: [u8; 32],
    ) -> Result<Self, EffectIntentControlErrorV1> {
        require_nonzero(result_commitment)?;
        require_nonzero(idempotency_commitment)?;
        Ok(Self {
            result_commitment,
            idempotency_commitment,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectIntentControlMutationV1 {
    ReserveDispatch {
        attempt: ExecutionAttemptOwnerV1,
        next_dispatch_fence: u64,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    RecoverReserved {
        attempt: ExecutionAttemptOwnerV1,
        dispatch_fence: u64,
    },
    RejectReserved {
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
    },
    RedispatchConclusiveNotApplied {
        attempt: ExecutionAttemptOwnerV1,
        next_dispatch_fence: u64,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    SealDispatch {
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
    },
    FinishDispatch {
        attempt: ExecutionAttemptOwnerV1,
        classification: RemoteClassificationV1,
        next_run_set_revision: u64,
    },
    RecoverSealedInDoubt {
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
    },
    BeginReconciliation {
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
        next_use_fence_commitment: [u8; 32],
    },
    RecordReconciliationRead {
        attempt: ExecutionAttemptOwnerV1,
        next_run_set_revision: u64,
    },
    FinishReconciliation {
        attempt: ExecutionAttemptOwnerV1,
        classification: RemoteClassificationV1,
        next_run_set_revision: u64,
        read_result_commitment: [u8; 32],
    },
    Withdraw {
        next_run_set_revision: u64,
    },
    MarkRecoveryRequired,
    MarkIntegrityBlocked,
    HandoffWriter(
        Box<(
            SameHomeWriterFencingReceiptV1,
            EffectIntentControlWriterTermV1,
        )>,
    ),
}

impl EffectIntentControlTransitionV1 {
    pub fn new(
        current_head: &EffectIntentControlHeadV1,
        current_revision: &EffectIntentControlRevisionV1,
        writer_term: EffectIntentControlWriterTermV1,
        mutation: EffectIntentControlMutationV1,
        action_request_id: ActionRequestIdV1,
    ) -> Result<Self, EffectIntentControlErrorV1> {
        validate_current_control(current_head, current_revision, writer_term)?;
        let contender = mutation.contender();
        let transition = Self {
            contender,
            intent: current_head.intent(),
            expected_head: current_head.id(),
            expected_revision: current_revision.id(),
            expected_writer_term: writer_term.id(),
            mutation,
            action_request_id,
        };
        Ok(transition)
    }

    pub const fn contender(&self) -> EffectIntentControlTransitionContenderV1 {
        self.contender
    }

    pub const fn action_request_id(&self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub const fn accepts_action(&self, action: ExecutionActionV1) -> bool {
        self.mutation.accepts_action(action)
    }

    pub fn candidate_revision(
        &self,
        current_revision: &EffectIntentControlRevisionV1,
        publication: Option<EffectIntentControlPublicationCommitmentsV1>,
    ) -> Result<EffectIntentControlRevisionV1, EffectIntentControlErrorV1> {
        if current_revision.id() != self.expected_revision
            || current_revision.intent() != self.intent
        {
            return Err(EffectIntentControlErrorV1::StaleExpectedHead);
        }
        derive_candidate_revision(current_revision, &self.mutation, publication)
    }

    pub const fn requires_store_publication(&self) -> bool {
        matches!(
            self.mutation,
            EffectIntentControlMutationV1::RejectReserved { .. }
                | EffectIntentControlMutationV1::FinishDispatch { .. }
                | EffectIntentControlMutationV1::RecoverSealedInDoubt { .. }
                | EffectIntentControlMutationV1::RecordReconciliationRead { .. }
                | EffectIntentControlMutationV1::FinishReconciliation { .. }
                | EffectIntentControlMutationV1::Withdraw { .. }
        )
    }

    pub fn meaning_value(&self) -> Result<CborValue, EffectIntentControlErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-control-transition-meaning.v1")?,
            bytes(self.intent.as_bytes()),
            bytes(self.expected_head.as_bytes()),
            bytes(self.expected_revision.as_bytes()),
            bytes(self.expected_writer_term.as_bytes()),
            CborValue::Unsigned(self.contender.tag()),
            self.mutation.canonical_value(),
            bytes(self.action_request_id.as_bytes()),
        ]))
    }

    pub fn apply(
        &self,
        current_head: &EffectIntentControlHeadV1,
        current_revision: &EffectIntentControlRevisionV1,
        writer_term: EffectIntentControlWriterTermV1,
        publication: Option<EffectIntentControlPublicationCommitmentsV1>,
    ) -> Result<
        (EffectIntentControlRevisionV1, EffectIntentControlHeadV1),
        EffectIntentControlErrorV1,
    > {
        if self.intent != current_head.intent()
            || self.intent != current_revision.intent()
            || current_head.id() != self.expected_head
            || current_head.revision() != self.expected_revision
            || current_revision.id() != self.expected_revision
            || current_head.writer_term() != self.expected_writer_term
            || writer_term.id() != self.expected_writer_term
            || current_head.home() != writer_term.home()
        {
            return Err(EffectIntentControlErrorV1::StaleExpectedHead);
        }
        let candidate = self.candidate_revision(current_revision, publication)?;
        let next_writer_term = match &self.mutation {
            EffectIntentControlMutationV1::HandoffWriter(handoff) => {
                let (receipt, successor) = handoff.as_ref();
                if receipt.intent() != self.intent
                    || receipt.home() != current_head.home()
                    || receipt.prior_head() != current_head.id()
                    || receipt.prior_writer_term() != writer_term.id()
                    || successor.intent() != self.intent
                    || successor.home() != current_head.home()
                    || successor.kind() != EffectIntentControlWriterTermKindV1::SameHomeRestore
                    || successor.prior_writer_term() != Some(writer_term.id())
                    || successor.fencing_receipt() != Some(receipt.id())
                {
                    return Err(EffectIntentControlErrorV1::OldWriterNotFenced);
                }
                *successor
            }
            _ => writer_term,
        };
        let head = EffectIntentControlHeadV1::new(
            self.intent,
            current_head.home(),
            &candidate,
            next_writer_term,
        )?;
        Ok((candidate, head))
    }
}

impl EffectIntentControlMutationV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::ReserveDispatch {
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
            } => mutation_value(
                1,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_dispatch_fence),
                    CborValue::Unsigned(*next_run_set_revision),
                    bytes(next_use_fence_commitment),
                ],
            ),
            Self::RecoverReserved {
                attempt,
                dispatch_fence,
            } => mutation_value(
                2,
                vec![owner_value(*attempt), CborValue::Unsigned(*dispatch_fence)],
            ),
            Self::RejectReserved {
                attempt,
                next_run_set_revision,
            } => mutation_value(
                3,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_run_set_revision),
                ],
            ),
            Self::RedispatchConclusiveNotApplied {
                attempt,
                next_dispatch_fence,
                next_run_set_revision,
                next_use_fence_commitment,
            } => mutation_value(
                4,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_dispatch_fence),
                    CborValue::Unsigned(*next_run_set_revision),
                    bytes(next_use_fence_commitment),
                ],
            ),
            Self::SealDispatch {
                attempt,
                next_run_set_revision,
            } => mutation_value(
                5,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_run_set_revision),
                ],
            ),
            Self::FinishDispatch {
                attempt,
                classification,
                next_run_set_revision,
            } => mutation_value(
                6,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(classification_tag(*classification)),
                    CborValue::Unsigned(*next_run_set_revision),
                ],
            ),
            Self::RecoverSealedInDoubt {
                attempt,
                next_run_set_revision,
            } => mutation_value(
                7,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_run_set_revision),
                ],
            ),
            Self::BeginReconciliation {
                attempt,
                next_run_set_revision,
                next_use_fence_commitment,
            } => mutation_value(
                8,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_run_set_revision),
                    bytes(next_use_fence_commitment),
                ],
            ),
            Self::RecordReconciliationRead {
                attempt,
                next_run_set_revision,
            } => mutation_value(
                14,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(*next_run_set_revision),
                ],
            ),
            Self::FinishReconciliation {
                attempt,
                classification,
                next_run_set_revision,
                read_result_commitment,
            } => mutation_value(
                9,
                vec![
                    owner_value(*attempt),
                    CborValue::Unsigned(classification_tag(*classification)),
                    CborValue::Unsigned(*next_run_set_revision),
                    bytes(read_result_commitment),
                ],
            ),
            Self::Withdraw {
                next_run_set_revision,
            } => mutation_value(10, vec![CborValue::Unsigned(*next_run_set_revision)]),
            Self::MarkRecoveryRequired => mutation_value(11, vec![]),
            Self::MarkIntegrityBlocked => mutation_value(12, vec![]),
            Self::HandoffWriter(handoff) => mutation_value(
                15,
                vec![
                    bytes(handoff.0.id().as_bytes()),
                    bytes(handoff.1.id().as_bytes()),
                ],
            ),
        }
    }

    const fn accepts_action(&self, action: ExecutionActionV1) -> bool {
        match self {
            Self::ReserveDispatch { .. }
            | Self::RecoverReserved { .. }
            | Self::RedispatchConclusiveNotApplied { .. } => matches!(
                action,
                ExecutionActionV1::OriginateEffectIntent
                    | ExecutionActionV1::OriginateCoordinationDelivery
                    | ExecutionActionV1::ReserveBootstrapMandateInteractionEffect
                    | ExecutionActionV1::ReserveContinuityMaintenanceEffect
            ),
            Self::RejectReserved { .. } => matches!(
                action,
                ExecutionActionV1::RecordDispatchOutcome
                    | ExecutionActionV1::PublishBootstrapMandateInteractionOutcome
                    | ExecutionActionV1::PublishContinuityMaintenanceEffectOutcome
            ),
            Self::SealDispatch { .. } | Self::FinishDispatch { .. } => matches!(
                action,
                ExecutionActionV1::RecordDispatchOutcome
                    | ExecutionActionV1::PublishBootstrapMandateInteractionOutcome
                    | ExecutionActionV1::PublishContinuityMaintenanceEffectOutcome
                    | ExecutionActionV1::OriginateEffectIntent
                    | ExecutionActionV1::OriginateCoordinationDelivery
                    | ExecutionActionV1::ReserveBootstrapMandateInteractionEffect
                    | ExecutionActionV1::ReserveContinuityMaintenanceEffect
            ),
            Self::RecoverSealedInDoubt { .. }
            | Self::BeginReconciliation { .. }
            | Self::RecordReconciliationRead { .. }
            | Self::FinishReconciliation { .. } => matches!(
                action,
                ExecutionActionV1::ReconcileEffectIntent
                    | ExecutionActionV1::ReconcileBootstrapMandateInteractionEffect
                    | ExecutionActionV1::ReconcileContinuityMaintenanceEffect
            ),
            Self::Withdraw { .. } => matches!(
                action,
                ExecutionActionV1::WithdrawEffectIntent
                    | ExecutionActionV1::WithdrawBootstrapMandateInteractionEffect
                    | ExecutionActionV1::WithdrawContinuityMaintenanceEffect
            ),
            Self::HandoffWriter(_) => true,
            Self::MarkRecoveryRequired | Self::MarkIntegrityBlocked => matches!(
                action,
                ExecutionActionV1::ReconcileEffectIntent
                    | ExecutionActionV1::ReconcileBootstrapMandateInteractionEffect
                    | ExecutionActionV1::ReconcileContinuityMaintenanceEffect
            ),
        }
    }

    const fn contender(&self) -> EffectIntentControlTransitionContenderV1 {
        match self {
            Self::ReserveDispatch { .. } => {
                EffectIntentControlTransitionContenderV1::OriginalHandler
            }
            Self::RecoverReserved { .. } | Self::RecoverSealedInDoubt { .. } => {
                EffectIntentControlTransitionContenderV1::RecoveryCaller
            }
            Self::RejectReserved { .. } => {
                EffectIntentControlTransitionContenderV1::PreSealLocalRejection
            }
            Self::RedispatchConclusiveNotApplied { .. } => {
                EffectIntentControlTransitionContenderV1::Redispatcher
            }
            Self::SealDispatch { .. } => EffectIntentControlTransitionContenderV1::Seal,
            Self::FinishDispatch { .. } => {
                EffectIntentControlTransitionContenderV1::ResponseHandler
            }
            Self::BeginReconciliation { .. }
            | Self::RecordReconciliationRead { .. }
            | Self::FinishReconciliation { .. } => {
                EffectIntentControlTransitionContenderV1::Reconciler
            }
            Self::Withdraw { .. } => EffectIntentControlTransitionContenderV1::Withdrawal,
            Self::MarkRecoveryRequired => EffectIntentControlTransitionContenderV1::RecoveryCaller,
            Self::MarkIntegrityBlocked => EffectIntentControlTransitionContenderV1::Terminalizer,
            Self::HandoffWriter(_) => {
                EffectIntentControlTransitionContenderV1::SameHomeRestoreWriter
            }
        }
    }
}

fn mutation_value(tag: u64, fields: Vec<CborValue>) -> CborValue {
    CborValue::Array(vec![CborValue::Unsigned(tag), CborValue::Array(fields)])
}

fn validate_current_control(
    current_head: &EffectIntentControlHeadV1,
    current_revision: &EffectIntentControlRevisionV1,
    writer_term: EffectIntentControlWriterTermV1,
) -> Result<(), EffectIntentControlErrorV1> {
    if current_head.intent() != current_revision.intent()
        || current_head.revision() != current_revision.id()
        || current_head.writer_term() != writer_term.id()
        || current_head.home() != writer_term.home()
    {
        return Err(EffectIntentControlErrorV1::StaleExpectedHead);
    }
    Ok(())
}

pub(crate) fn derive_candidate_revision(
    current: &EffectIntentControlRevisionV1,
    mutation: &EffectIntentControlMutationV1,
    publication: Option<EffectIntentControlPublicationCommitmentsV1>,
) -> Result<EffectIntentControlRevisionV1, EffectIntentControlErrorV1> {
    let begins_recovery_reconciliation = matches!(
        mutation,
        EffectIntentControlMutationV1::BeginReconciliation { .. }
    ) && current.health()
        == EffectIntentControlHealthV1::RecoveryRequired
        && current.live_attempt().is_none()
        && current.live_dispatch() == EffectIntentLiveDispatchV1::None
        && current.classification() == RemoteClassificationV1::InDoubt
        && current.runs_closed();
    if current.health() != EffectIntentControlHealthV1::Healthy
        && !begins_recovery_reconciliation
        && matches!(
            mutation,
            EffectIntentControlMutationV1::ReserveDispatch { .. }
                | EffectIntentControlMutationV1::RecoverReserved { .. }
                | EffectIntentControlMutationV1::RejectReserved { .. }
                | EffectIntentControlMutationV1::RedispatchConclusiveNotApplied { .. }
                | EffectIntentControlMutationV1::SealDispatch { .. }
                | EffectIntentControlMutationV1::FinishDispatch { .. }
                | EffectIntentControlMutationV1::RecoverSealedInDoubt { .. }
                | EffectIntentControlMutationV1::BeginReconciliation { .. }
                | EffectIntentControlMutationV1::RecordReconciliationRead { .. }
                | EffectIntentControlMutationV1::FinishReconciliation { .. }
                | EffectIntentControlMutationV1::Withdraw { .. }
        )
    {
        return Err(EffectIntentControlErrorV1::IllegalControlTransition);
    }
    let requires_publication = matches!(
        mutation,
        EffectIntentControlMutationV1::RejectReserved { .. }
            | EffectIntentControlMutationV1::FinishDispatch { .. }
            | EffectIntentControlMutationV1::RecoverSealedInDoubt { .. }
            | EffectIntentControlMutationV1::RecordReconciliationRead { .. }
            | EffectIntentControlMutationV1::FinishReconciliation { .. }
            | EffectIntentControlMutationV1::Withdraw { .. }
    );
    if requires_publication != publication.is_some() {
        return Err(EffectIntentControlErrorV1::MissingStorePublicationCommitment);
    }
    let mut next = current.parts();
    match mutation {
        EffectIntentControlMutationV1::ReserveDispatch {
            attempt,
            next_dispatch_fence,
            next_run_set_revision,
            next_use_fence_commitment,
        } => {
            if !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_attempt().is_some()
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || !matches!(
                    current.classification(),
                    RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
                )
                || !current.runs_closed()
                || *next_dispatch_fence
                    != current
                        .dispatch_fence_high_water()
                        .checked_add(1)
                        .ok_or(EffectIntentControlErrorV1::MonotonicCounterOverflow)?
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            require_nonzero(*next_use_fence_commitment)?;
            next.attempt_history.push(*attempt);
            next.live_attempt = Some(*attempt);
            next.live_dispatch = EffectIntentLiveDispatchV1::Reserved;
            next.classification = RemoteClassificationV1::Dispatching;
            next.dispatch_fence_high_water = *next_dispatch_fence;
            next.run_set_revision = *next_run_set_revision;
            next.use_fence_commitment = *next_use_fence_commitment;
            next.runs_closed = false;
            next.result_commitment = None;
            next.idempotency_commitment = None;
        }
        EffectIntentControlMutationV1::RecoverReserved {
            attempt,
            dispatch_fence,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::Reserved
                || current.classification() != RemoteClassificationV1::Dispatching
                || current.runs_closed()
                || current.dispatch_fence_high_water() != *dispatch_fence
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
        }
        EffectIntentControlMutationV1::RejectReserved {
            attempt,
            next_run_set_revision,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::Reserved
                || current.classification() != RemoteClassificationV1::Dispatching
                || current.runs_closed()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            let publication = require_terminal_publication(publication)?;
            next.live_attempt = None;
            next.live_dispatch = EffectIntentLiveDispatchV1::None;
            next.classification = RemoteClassificationV1::ConfirmedNotApplied;
            next.run_set_revision = *next_run_set_revision;
            next.runs_closed = true;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
        }
        EffectIntentControlMutationV1::RedispatchConclusiveNotApplied {
            attempt,
            next_dispatch_fence,
            next_run_set_revision,
            next_use_fence_commitment,
        } => {
            if !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_attempt().is_some()
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || current.classification() != RemoteClassificationV1::ConfirmedNotApplied
                || !current.runs_closed()
                || *next_dispatch_fence
                    != current
                        .dispatch_fence_high_water()
                        .checked_add(1)
                        .ok_or(EffectIntentControlErrorV1::MonotonicCounterOverflow)?
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            require_nonzero(*next_use_fence_commitment)?;
            next.attempt_history.push(*attempt);
            next.live_attempt = Some(*attempt);
            next.live_dispatch = EffectIntentLiveDispatchV1::Reserved;
            next.classification = RemoteClassificationV1::Dispatching;
            next.dispatch_fence_high_water = *next_dispatch_fence;
            next.run_set_revision = *next_run_set_revision;
            next.use_fence_commitment = *next_use_fence_commitment;
            next.runs_closed = false;
            next.result_commitment = None;
            next.idempotency_commitment = None;
        }
        EffectIntentControlMutationV1::SealDispatch {
            attempt,
            next_run_set_revision,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::Reserved
                || current.classification() != RemoteClassificationV1::Dispatching
                || current.runs_closed()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            next.live_dispatch = EffectIntentLiveDispatchV1::Sealed;
            next.classification = RemoteClassificationV1::InDoubt;
            next.run_set_revision = *next_run_set_revision;
        }
        EffectIntentControlMutationV1::FinishDispatch {
            attempt,
            classification,
            next_run_set_revision,
        } => {
            let reserved_finish = current.live_dispatch() == EffectIntentLiveDispatchV1::Reserved
                && *classification == RemoteClassificationV1::ConfirmedNotApplied;
            let sealed_finish = current.live_dispatch() == EffectIntentLiveDispatchV1::Sealed
                && matches!(
                    classification,
                    RemoteClassificationV1::Pending
                        | RemoteClassificationV1::InDoubt
                        | RemoteClassificationV1::ConfirmedApplied
                        | RemoteClassificationV1::ConfirmedNotApplied
                        | RemoteClassificationV1::PartiallyApplied
                        | RemoteClassificationV1::Conflicted
                );
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.runs_closed()
                || !(reserved_finish || sealed_finish)
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            let publication = require_terminal_publication(publication)?;
            next.live_attempt = None;
            next.live_dispatch = EffectIntentLiveDispatchV1::None;
            next.classification = *classification;
            next.run_set_revision = *next_run_set_revision;
            next.runs_closed = true;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
        }
        EffectIntentControlMutationV1::RecoverSealedInDoubt {
            attempt,
            next_run_set_revision,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Dispatch(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::Sealed
                || current.classification() != RemoteClassificationV1::InDoubt
                || current.runs_closed()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            let publication = require_terminal_publication(publication)?;
            next.live_attempt = None;
            next.live_dispatch = EffectIntentLiveDispatchV1::None;
            next.run_set_revision = *next_run_set_revision;
            next.runs_closed = true;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
            next.health = EffectIntentControlHealthV1::RecoveryRequired;
        }
        EffectIntentControlMutationV1::BeginReconciliation {
            attempt,
            next_run_set_revision,
            next_use_fence_commitment,
        } => {
            if !matches!(attempt, ExecutionAttemptOwnerV1::Reconciliation(_))
                || current.live_attempt().is_some()
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || !matches!(
                    current.classification(),
                    RemoteClassificationV1::Pending
                        | RemoteClassificationV1::InDoubt
                        | RemoteClassificationV1::PartiallyApplied
                        | RemoteClassificationV1::Conflicted
                )
                || !current.runs_closed()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            require_nonzero(*next_use_fence_commitment)?;
            next.attempt_history.push(*attempt);
            next.live_attempt = Some(*attempt);
            next.run_set_revision = *next_run_set_revision;
            next.use_fence_commitment = *next_use_fence_commitment;
            next.runs_closed = false;
            next.result_commitment = None;
            next.idempotency_commitment = None;
            if begins_recovery_reconciliation {
                next.health = EffectIntentControlHealthV1::Healthy;
            }
        }
        EffectIntentControlMutationV1::RecordReconciliationRead {
            attempt,
            next_run_set_revision,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Reconciliation(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || current.runs_closed()
                || current.result_commitment.is_some()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_run_revision_delta(current, *next_run_set_revision, 2)?;
            let publication = require_terminal_publication(publication)?;
            next.run_set_revision = *next_run_set_revision;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
        }
        EffectIntentControlMutationV1::FinishReconciliation {
            attempt,
            classification,
            next_run_set_revision,
            read_result_commitment,
        } => {
            if current.live_attempt() != Some(*attempt)
                || !matches!(attempt, ExecutionAttemptOwnerV1::Reconciliation(_))
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || current.runs_closed()
                || matches!(
                    classification,
                    RemoteClassificationV1::Prepared | RemoteClassificationV1::Dispatching
                )
                || current.result_commitment != Some(*read_result_commitment)
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            if current.run_set_revision() != *next_run_set_revision {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            let publication = require_terminal_publication(publication)?;
            next.live_attempt = None;
            next.classification = *classification;
            next.run_set_revision = *next_run_set_revision;
            next.runs_closed = true;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
        }
        EffectIntentControlMutationV1::Withdraw {
            next_run_set_revision,
        } => {
            if current.live_attempt().is_some()
                || current.live_dispatch() != EffectIntentLiveDispatchV1::None
                || !matches!(
                    current.classification(),
                    RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
                )
                || !current.runs_closed()
            {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            require_next_run_revision(current, *next_run_set_revision)?;
            let publication = require_terminal_publication(publication)?;
            next.classification = RemoteClassificationV1::Cancelled;
            next.run_set_revision = *next_run_set_revision;
            next.result_commitment = Some(publication.result_commitment);
            next.idempotency_commitment = Some(publication.idempotency_commitment);
        }
        EffectIntentControlMutationV1::MarkRecoveryRequired => {
            if current.health() != EffectIntentControlHealthV1::Healthy {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            next.health = EffectIntentControlHealthV1::RecoveryRequired;
        }
        EffectIntentControlMutationV1::MarkIntegrityBlocked => {
            if current.health() == EffectIntentControlHealthV1::IntegrityBlocked {
                return Err(EffectIntentControlErrorV1::IllegalControlTransition);
            }
            next.health = EffectIntentControlHealthV1::IntegrityBlocked;
        }
        EffectIntentControlMutationV1::HandoffWriter(_)
            if current.health() == EffectIntentControlHealthV1::IntegrityBlocked =>
        {
            return Err(EffectIntentControlErrorV1::IllegalControlTransition);
        }
        EffectIntentControlMutationV1::HandoffWriter(_) => {
            if current.health() == EffectIntentControlHealthV1::RecoveryRequired {
                next.health = EffectIntentControlHealthV1::Healthy;
            }
        }
    }
    EffectIntentControlRevisionV1::new(next)
}

fn require_terminal_publication(
    publication: Option<EffectIntentControlPublicationCommitmentsV1>,
) -> Result<EffectIntentControlPublicationCommitmentsV1, EffectIntentControlErrorV1> {
    publication.ok_or(EffectIntentControlErrorV1::MissingStorePublicationCommitment)
}

fn require_next_run_revision(
    current: &EffectIntentControlRevisionV1,
    next: u64,
) -> Result<(), EffectIntentControlErrorV1> {
    if next
        != current
            .run_set_revision()
            .checked_add(1)
            .ok_or(EffectIntentControlErrorV1::MonotonicCounterOverflow)?
    {
        return Err(EffectIntentControlErrorV1::IllegalControlTransition);
    }
    Ok(())
}

fn require_run_revision_delta(
    current: &EffectIntentControlRevisionV1,
    next: u64,
    delta: u64,
) -> Result<(), EffectIntentControlErrorV1> {
    if next
        != current
            .run_set_revision()
            .checked_add(delta)
            .ok_or(EffectIntentControlErrorV1::MonotonicCounterOverflow)?
    {
        return Err(EffectIntentControlErrorV1::IllegalControlTransition);
    }
    Ok(())
}

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

    const fn tag(self) -> u64 {
        match self {
            Self::OriginalHandler => 1,
            Self::RecoveryCaller => 2,
            Self::PreSealLocalRejection => 3,
            Self::Seal => 4,
            Self::ResponseHandler => 5,
            Self::Terminalizer => 6,
            Self::Classifier => 7,
            Self::Reconciler => 8,
            Self::Redispatcher => 9,
            Self::Withdrawal => 10,
            Self::SameHomeRestoreWriter => 11,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentControlConsumerDispositionV1 {
    CandidateContractDefinition,
    CandidateProofReader,
    SealedV1AuditMigrationConsumer,
}

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

pub fn legal_live_dispatch_classification(
    live_dispatch: EffectIntentLiveDispatchV1,
    classification: RemoteClassificationV1,
) -> bool {
    match live_dispatch {
        EffectIntentLiveDispatchV1::None => classification != RemoteClassificationV1::Dispatching,
        EffectIntentLiveDispatchV1::Reserved => {
            classification == RemoteClassificationV1::Dispatching
        }
        EffectIntentLiveDispatchV1::Sealed => classification == RemoteClassificationV1::InDoubt,
    }
}

fn control_revision_payload(
    parts: &EffectIntentControlRevisionPartsV1,
) -> Result<CborValue, EffectIntentControlErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-control-revision.v1")?,
        bytes(parts.intent.as_bytes()),
        CborValue::Array(
            parts
                .attempt_history
                .iter()
                .copied()
                .map(owner_value)
                .collect(),
        ),
        CborValue::optional(parts.live_attempt.map(owner_value)),
        CborValue::Unsigned(live_dispatch_tag(parts.live_dispatch)),
        CborValue::Unsigned(classification_tag(parts.classification)),
        CborValue::Unsigned(parts.dispatch_fence_high_water),
        CborValue::Unsigned(parts.run_set_revision),
        CborValue::Bool(parts.runs_closed),
        bytes(&parts.material_commitment),
        bytes(&parts.credential_commitment),
        bytes(&parts.use_fence_commitment),
        CborValue::optional(parts.result_commitment.map(|value| bytes(&value))),
        CborValue::optional(parts.idempotency_commitment.map(|value| bytes(&value))),
        CborValue::Unsigned(parts.health.tag()),
    ]))
}

fn owner_value(owner: ExecutionAttemptOwnerV1) -> CborValue {
    match owner {
        ExecutionAttemptOwnerV1::Step(id) => {
            CborValue::Array(vec![CborValue::Unsigned(1), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Dispatch(id) => {
            CborValue::Array(vec![CborValue::Unsigned(2), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Reconciliation(id) => {
            CborValue::Array(vec![CborValue::Unsigned(3), bytes(id.as_bytes())])
        }
    }
}

fn live_dispatch_tag(value: EffectIntentLiveDispatchV1) -> u64 {
    match value {
        EffectIntentLiveDispatchV1::None => 1,
        EffectIntentLiveDispatchV1::Reserved => 2,
        EffectIntentLiveDispatchV1::Sealed => 3,
    }
}

fn classification_tag(value: RemoteClassificationV1) -> u64 {
    match value {
        RemoteClassificationV1::Prepared => 1,
        RemoteClassificationV1::Dispatching => 2,
        RemoteClassificationV1::Pending => 3,
        RemoteClassificationV1::InDoubt => 4,
        RemoteClassificationV1::ConfirmedApplied => 5,
        RemoteClassificationV1::ConfirmedNotApplied => 6,
        RemoteClassificationV1::PartiallyApplied => 7,
        RemoteClassificationV1::Conflicted => 8,
        RemoteClassificationV1::Cancelled => 9,
    }
}

fn parse_live_dispatch(tag: u64) -> Result<EffectIntentLiveDispatchV1, EffectIntentControlErrorV1> {
    match tag {
        1 => Ok(EffectIntentLiveDispatchV1::None),
        2 => Ok(EffectIntentLiveDispatchV1::Reserved),
        3 => Ok(EffectIntentLiveDispatchV1::Sealed),
        _ => Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
    }
}

fn parse_classification(tag: u64) -> Result<RemoteClassificationV1, EffectIntentControlErrorV1> {
    match tag {
        1 => Ok(RemoteClassificationV1::Prepared),
        2 => Ok(RemoteClassificationV1::Dispatching),
        3 => Ok(RemoteClassificationV1::Pending),
        4 => Ok(RemoteClassificationV1::InDoubt),
        5 => Ok(RemoteClassificationV1::ConfirmedApplied),
        6 => Ok(RemoteClassificationV1::ConfirmedNotApplied),
        7 => Ok(RemoteClassificationV1::PartiallyApplied),
        8 => Ok(RemoteClassificationV1::Conflicted),
        9 => Ok(RemoteClassificationV1::Cancelled),
        _ => Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
    }
}

fn parse_optional_control_owner(
    value: &CborValue,
) -> Result<Option<ExecutionAttemptOwnerV1>, EffectIntentControlErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), value] = fields.as_slice() else {
                return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
            };
            Ok(Some(super::runtime::parse_attempt_owner(value).map_err(
                |_| EffectIntentControlErrorV1::InvalidStoredControlCarrier,
            )?))
        }
        _ => Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
    }
}

fn parse_optional_control_digest(
    value: &CborValue,
) -> Result<Option<[u8; 32]>, EffectIntentControlErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), value] = fields.as_slice() else {
                return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
            };
            Ok(Some(exact_control_digest(value)?))
        }
        _ => Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier),
    }
}

fn parse_optional_control_token(
    value: &CborValue,
) -> Result<Option<EffectIntentControlTokenV1>, EffectIntentControlErrorV1> {
    Ok(parse_optional_control_digest(value)?
        .map(|digest| EffectIntentControlTokenV1::new(HomeTokenV1::new(digest))))
}

fn exact_control_digest(value: &CborValue) -> Result<[u8; 32], EffectIntentControlErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(EffectIntentControlErrorV1::InvalidStoredControlCarrier);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| EffectIntentControlErrorV1::InvalidStoredControlCarrier)
}

fn require_nonzero(value: [u8; 32]) -> Result<(), EffectIntentControlErrorV1> {
    if value == [0; 32] {
        Err(EffectIntentControlErrorV1::MissingCommitment)
    } else {
        Ok(())
    }
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectIntentControlErrorV1 {
    #[error("Effect Intent control commitment must not be all zero")]
    MissingCommitment,
    #[error("Effect Intent control product is not one of the ten legal pairs")]
    InvalidControlProduct,
    #[error("Effect Intent Attempt history or live Attempt is incoherent")]
    InvalidAttemptClosure,
    #[error("Effect Intent control revision belongs to another Intent")]
    IntentMismatch,
    #[error("Effect Intent expected-old Head, revision, or writer term is stale")]
    StaleExpectedHead,
    #[error("same-home restore requires conclusive old-writer fencing")]
    OldWriterNotFenced,
    #[error("Effect Intent control transition is not one of the closed legal mutations")]
    IllegalControlTransition,
    #[error("Effect Intent control monotonic counter overflowed")]
    MonotonicCounterOverflow,
    #[error("terminal Effect Intent control mutation lacks Store-derived publication commitments")]
    MissingStorePublicationCommitment,
    #[error("stored Effect Intent control carrier is malformed or non-canonical")]
    InvalidStoredControlCarrier,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::execution::runtime::{
        DispatchAttemptIdV1, EffectIntentIdV1, ReconciliationAttemptIdV1,
    };

    fn token(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    #[test]
    fn control_pair_product_is_exactly_ten() {
        let classifications = [
            RemoteClassificationV1::Prepared,
            RemoteClassificationV1::Dispatching,
            RemoteClassificationV1::Pending,
            RemoteClassificationV1::InDoubt,
            RemoteClassificationV1::ConfirmedApplied,
            RemoteClassificationV1::ConfirmedNotApplied,
            RemoteClassificationV1::PartiallyApplied,
            RemoteClassificationV1::Conflicted,
            RemoteClassificationV1::Cancelled,
        ];
        let count = [
            EffectIntentLiveDispatchV1::None,
            EffectIntentLiveDispatchV1::Reserved,
            EffectIntentLiveDispatchV1::Sealed,
        ]
        .into_iter()
        .flat_map(|live| {
            classifications
                .into_iter()
                .map(move |classification| (live, classification))
        })
        .filter(|(live, classification)| legal_live_dispatch_classification(*live, *classification))
        .count();
        assert_eq!(count, 10);
    }

    #[test]
    fn same_home_restore_is_bound_to_a_durable_fencing_receipt() {
        let intent = EffectIntentIdV1::derive("intent").unwrap();
        let home = HomeTokenV1::new(token(1));
        let original = EffectIntentControlWriterTermV1::originate(intent, home, token(2)).unwrap();
        let prior_head = EffectIntentControlTokenV1::new(HomeTokenV1::new(token(3)));
        let receipt = SameHomeWriterFencingReceiptV1::issue(
            intent,
            home,
            prior_head,
            original.id(),
            token(4),
            token(5),
            2,
        )
        .unwrap();
        let restored =
            EffectIntentControlWriterTermV1::same_home_restore(receipt, token(6)).unwrap();
        assert_eq!(restored.home(), home);
        assert_eq!(restored.prior_writer_term, Some(original.id()));
        assert_eq!(restored.fencing_receipt, Some(receipt.id()));
    }

    #[test]
    fn unhealthy_control_products_fail_closed_until_exact_recovery() {
        let intent = EffectIntentIdV1::derive("health-matrix-intent").unwrap();
        let healthy = EffectIntentControlRevisionV1::new(EffectIntentControlRevisionPartsV1 {
            intent,
            attempt_history: Vec::new(),
            live_attempt: None,
            live_dispatch: EffectIntentLiveDispatchV1::None,
            classification: RemoteClassificationV1::Prepared,
            dispatch_fence_high_water: 0,
            run_set_revision: 1,
            runs_closed: true,
            material_commitment: token(11),
            credential_commitment: token(12),
            use_fence_commitment: token(13),
            result_commitment: None,
            idempotency_commitment: None,
            health: EffectIntentControlHealthV1::Healthy,
        })
        .unwrap();
        let dispatch =
            ExecutionAttemptOwnerV1::Dispatch(DispatchAttemptIdV1::from_bytes(token(14)).unwrap());
        let reconciliation = ExecutionAttemptOwnerV1::Reconciliation(
            ReconciliationAttemptIdV1::from_bytes(token(15)).unwrap(),
        );
        let behavior_mutations = || {
            vec![
                EffectIntentControlMutationV1::ReserveDispatch {
                    attempt: dispatch,
                    next_dispatch_fence: 1,
                    next_run_set_revision: 2,
                    next_use_fence_commitment: token(16),
                },
                EffectIntentControlMutationV1::RecoverReserved {
                    attempt: dispatch,
                    dispatch_fence: 1,
                },
                EffectIntentControlMutationV1::RejectReserved {
                    attempt: dispatch,
                    next_run_set_revision: 2,
                },
                EffectIntentControlMutationV1::RedispatchConclusiveNotApplied {
                    attempt: dispatch,
                    next_dispatch_fence: 1,
                    next_run_set_revision: 2,
                    next_use_fence_commitment: token(16),
                },
                EffectIntentControlMutationV1::SealDispatch {
                    attempt: dispatch,
                    next_run_set_revision: 2,
                },
                EffectIntentControlMutationV1::FinishDispatch {
                    attempt: dispatch,
                    classification: RemoteClassificationV1::ConfirmedNotApplied,
                    next_run_set_revision: 2,
                },
                EffectIntentControlMutationV1::RecoverSealedInDoubt {
                    attempt: dispatch,
                    next_run_set_revision: 2,
                },
                EffectIntentControlMutationV1::BeginReconciliation {
                    attempt: reconciliation,
                    next_run_set_revision: 2,
                    next_use_fence_commitment: token(17),
                },
                EffectIntentControlMutationV1::RecordReconciliationRead {
                    attempt: reconciliation,
                    next_run_set_revision: 3,
                },
                EffectIntentControlMutationV1::FinishReconciliation {
                    attempt: reconciliation,
                    classification: RemoteClassificationV1::ConfirmedNotApplied,
                    next_run_set_revision: 3,
                    read_result_commitment: token(18),
                },
                EffectIntentControlMutationV1::Withdraw {
                    next_run_set_revision: 2,
                },
            ]
        };
        let publication = EffectIntentControlPublicationCommitmentsV1::from_store_publication(
            token(19),
            token(20),
        )
        .unwrap();

        let recovery_required = derive_candidate_revision(
            &healthy,
            &EffectIntentControlMutationV1::MarkRecoveryRequired,
            None,
        )
        .unwrap();
        assert_eq!(
            recovery_required.health(),
            EffectIntentControlHealthV1::RecoveryRequired
        );
        for mutation in behavior_mutations() {
            let before = recovery_required.clone();
            assert_eq!(
                derive_candidate_revision(&recovery_required, &mutation, Some(publication)),
                Err(EffectIntentControlErrorV1::IllegalControlTransition)
            );
            assert_eq!(recovery_required, before);
        }

        let home = HomeTokenV1::new(token(21));
        let writer = EffectIntentControlWriterTermV1::originate(intent, home, token(22)).unwrap();
        let prior_head = EffectIntentControlTokenV1::new(HomeTokenV1::new(token(23)));
        let receipt = SameHomeWriterFencingReceiptV1::issue(
            intent,
            home,
            prior_head,
            writer.id(),
            token(24),
            token(25),
            2,
        )
        .unwrap();
        let successor =
            EffectIntentControlWriterTermV1::same_home_restore(receipt, token(26)).unwrap();
        let handoff = EffectIntentControlMutationV1::HandoffWriter(Box::new((receipt, successor)));
        let handoff_candidate =
            derive_candidate_revision(&recovery_required, &handoff, None).unwrap();
        assert_eq!(
            handoff_candidate.health(),
            EffectIntentControlHealthV1::Healthy
        );

        let integrity_blocked = derive_candidate_revision(
            &healthy,
            &EffectIntentControlMutationV1::MarkIntegrityBlocked,
            None,
        )
        .unwrap();
        assert_eq!(
            integrity_blocked.health(),
            EffectIntentControlHealthV1::IntegrityBlocked
        );
        for mutation in behavior_mutations() {
            let before = integrity_blocked.clone();
            assert_eq!(
                derive_candidate_revision(&integrity_blocked, &mutation, Some(publication)),
                Err(EffectIntentControlErrorV1::IllegalControlTransition)
            );
            assert_eq!(integrity_blocked, before);
        }
        let before_handoff = integrity_blocked.clone();
        assert_eq!(
            derive_candidate_revision(&integrity_blocked, &handoff, None),
            Err(EffectIntentControlErrorV1::IllegalControlTransition)
        );
        assert_eq!(integrity_blocked, before_handoff);
    }
}
