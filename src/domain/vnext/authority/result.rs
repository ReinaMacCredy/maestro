use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::closed::{ActionAuthorityBasisKindV1, AuthorityTagError};
use super::identity::{
    ActionRequestIdV1, ActionResultIdV1, AuthorityContextIdV1, AuthorizationReceiptIdV1,
    EffectReferenceIdV1, StateTokenIdV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ActionOutcomeV1 {
    Committed = 1,
    NoOp = 2,
    Rejected = 3,
    Stale = 4,
    Conflict = 5,
    Unavailable = 6,
    InDoubt = 7,
}

impl ActionOutcomeV1 {
    pub const ALL: [Self; 7] = [
        Self::Committed,
        Self::NoOp,
        Self::Rejected,
        Self::Stale,
        Self::Conflict,
        Self::Unavailable,
        Self::InDoubt,
    ];
}

impl TryFrom<u8> for ActionOutcomeV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::Committed),
            2 => Ok(Self::NoOp),
            3 => Ok(Self::Rejected),
            4 => Ok(Self::Stale),
            5 => Ok(Self::Conflict),
            6 => Ok(Self::Unavailable),
            7 => Ok(Self::InDoubt),
            value => Err(AuthorityTagError::UnknownActionOutcome(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationReceiptV1 {
    id: AuthorizationReceiptIdV1,
    request_id: ActionRequestIdV1,
    context_id: AuthorityContextIdV1,
    basis_kind: ActionAuthorityBasisKindV1,
    prior_state_token: StateTokenIdV1,
    resulting_state_token: StateTokenIdV1,
}

impl AuthorizationReceiptV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.authorization-receipt-value.v1";

    pub fn new(
        request_id: ActionRequestIdV1,
        context_id: AuthorityContextIdV1,
        basis_kind: ActionAuthorityBasisKindV1,
        prior_state_token: StateTokenIdV1,
        resulting_state_token: StateTokenIdV1,
    ) -> Result<Self, CborError> {
        let mut receipt = Self {
            id: AuthorizationReceiptIdV1::from_digest([0; 32]),
            request_id,
            context_id,
            basis_kind,
            prior_state_token,
            resulting_state_token,
        };
        receipt.id = AuthorizationReceiptIdV1::from_digest(hash(&receipt.schema_value()?)?);
        Ok(receipt)
    }

    pub const fn id(&self) -> AuthorizationReceiptIdV1 {
        self.id
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn basis_kind(&self) -> ActionAuthorityBasisKindV1 {
        self.basis_kind
    }

    pub const fn prior_state_token(&self) -> StateTokenIdV1 {
        self.prior_state_token
    }

    pub const fn resulting_state_token(&self) -> StateTokenIdV1 {
        self.resulting_state_token
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    pub const fn schema_domain(&self) -> &'static str {
        Self::SCHEMA_DOMAIN
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.request_id.as_bytes().to_vec()),
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.basis_kind as u64),
            CborValue::Bytes(self.prior_state_token.as_bytes().to_vec()),
            CborValue::Bytes(self.resulting_state_token.as_bytes().to_vec()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ActionResultError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = &value else {
            return Err(ActionResultError::InvalidAuthorizationReceipt);
        };
        let [
            CborValue::Text(domain),
            CborValue::Bytes(request_id),
            CborValue::Bytes(context_id),
            CborValue::Unsigned(basis_kind),
            CborValue::Bytes(prior_state_token),
            CborValue::Bytes(resulting_state_token),
        ] = fields.as_slice()
        else {
            return Err(ActionResultError::InvalidAuthorizationReceipt);
        };
        if domain != Self::SCHEMA_DOMAIN {
            return Err(ActionResultError::InvalidAuthorizationReceipt);
        }
        let receipt = Self::new(
            ActionRequestIdV1::from_digest(exact_digest(request_id)?),
            AuthorityContextIdV1::from_digest(exact_digest(context_id)?),
            ActionAuthorityBasisKindV1::try_from(
                u8::try_from(*basis_kind)
                    .map_err(|_| ActionResultError::InvalidAuthorizationReceipt)?,
            )?,
            StateTokenIdV1::from_digest(exact_digest(prior_state_token)?),
            StateTokenIdV1::from_digest(exact_digest(resulting_state_token)?),
        )?;
        if receipt.canonical_bytes()? != bytes {
            return Err(ActionResultError::InvalidAuthorizationReceipt);
        }
        Ok(receipt)
    }
}

fn exact_digest(value: &[u8]) -> Result<[u8; 32], ActionResultError> {
    value
        .try_into()
        .map_err(|_| ActionResultError::InvalidAuthorizationReceipt)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseOriginV1 {
    Fresh,
    Replay {
        original_result_id: ActionResultIdV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionResultV1 {
    id: ActionResultIdV1,
    request_id: ActionRequestIdV1,
    outcome: ActionOutcomeV1,
    authorization_receipt: Option<AuthorizationReceiptV1>,
    effect_reference: Option<EffectReferenceIdV1>,
    response_origin: ResponseOriginV1,
}

impl ActionResultV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.action-result-value.v1";

    pub fn new(
        request_id: ActionRequestIdV1,
        outcome: ActionOutcomeV1,
        authorization_receipt: Option<AuthorizationReceiptV1>,
        effect_reference: Option<EffectReferenceIdV1>,
    ) -> Result<Self, ActionResultError> {
        if outcome == ActionOutcomeV1::Committed && authorization_receipt.is_none() {
            return Err(ActionResultError::CommittedRequiresAuthorizationReceipt);
        }
        if authorization_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.request_id() != request_id)
        {
            return Err(ActionResultError::AuthorizationReceiptRequestMismatch);
        }
        if outcome == ActionOutcomeV1::InDoubt && effect_reference.is_none() {
            return Err(ActionResultError::InDoubtRequiresEffectReference);
        }
        if outcome != ActionOutcomeV1::InDoubt && effect_reference.is_some() {
            return Err(ActionResultError::UnexpectedEffectReference);
        }
        let mut result = Self {
            id: ActionResultIdV1::from_digest([0; 32]),
            request_id,
            outcome,
            authorization_receipt,
            effect_reference,
            response_origin: ResponseOriginV1::Fresh,
        };
        result.id = ActionResultIdV1::from_digest(hash(&result.schema_value()?)?);
        Ok(result)
    }

    pub const fn id(&self) -> ActionResultIdV1 {
        self.id
    }

    pub const fn outcome(&self) -> ActionOutcomeV1 {
        self.outcome
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub fn authorization_receipt(&self) -> Option<&AuthorizationReceiptV1> {
        self.authorization_receipt.as_ref()
    }

    pub const fn effect_reference(&self) -> Option<EffectReferenceIdV1> {
        self.effect_reference
    }

    pub const fn response_origin(&self) -> ResponseOriginV1 {
        self.response_origin
    }

    pub fn replay(&self) -> Self {
        Self {
            response_origin: ResponseOriginV1::Replay {
                original_result_id: self.id,
            },
            ..self.clone()
        }
    }

    pub const fn schema_domain(&self) -> &'static str {
        Self::SCHEMA_DOMAIN
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.request_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.outcome as u64),
            CborValue::optional(
                self.authorization_receipt
                    .as_ref()
                    .map(|receipt| CborValue::Bytes(receipt.id().as_bytes().to_vec())),
            ),
            CborValue::optional(
                self.effect_reference
                    .map(|reference| CborValue::Bytes(reference.as_bytes().to_vec())),
            ),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ActionResultError {
    #[error("stored Authorization Receipt is malformed or non-canonical")]
    InvalidAuthorizationReceipt,
    #[error("committed Action Result requires an Authorization Receipt")]
    CommittedRequiresAuthorizationReceipt,
    #[error("Action Result and Authorization Receipt must bind the same request")]
    AuthorizationReceiptRequestMismatch,
    #[error("in_doubt Action Result requires an exact effect reference")]
    InDoubtRequiresEffectReference,
    #[error("only in_doubt Action Result may contain an effect reference")]
    UnexpectedEffectReference,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    AuthorityTag(#[from] AuthorityTagError),
}
