use sha2::{Digest, Sha256};
use thiserror::Error;

pub const RECIPE_IDS_V1: [&str; 10] = [
    "bounded-continuation",
    "conflict-handoff",
    "design-relay",
    "fanout",
    "intake-triage",
    "learning",
    "setup",
    "ship",
    "synthesize",
    "wayfinding",
];
pub const PRIMARY_RECIPE_IDS_V1: [&str; 9] = [
    "conflict-handoff",
    "design-relay",
    "fanout",
    "intake-triage",
    "learning",
    "setup",
    "ship",
    "synthesize",
    "wayfinding",
];
pub const BOUNDED_CONTINUATION_PROFILE_IDS_V1: [&str; 2] = ["attended", "unattended"];
pub const RECIPE_PHASES_V1: [&str; 6] =
    ["Perceive", "Choose", "Act", "Observe", "Learn", "Continue"];
pub const RECIPE_MANIFEST_FIELD_NAMES_V1: [&str; 14] = [
    "recipe_id",
    "semantic_version",
    "recipe_role",
    "purpose_guidance_resource_ref",
    "trigger_projection_reason_refs",
    "required_contract_refs",
    "restrictive_operation_filter_program_ref",
    "phase_guidance_resource_refs",
    "required_projection_predicate_refs",
    "hard_stop_predicate_refs",
    "completion_predicate_refs",
    "return_reason_refs",
    "operating_limit_program_ref",
    "allowed_continuation_profile_refs",
];
pub const RECIPE_SELECTION_REQUEST_FIELD_NAMES_V1: [&str; 4] = [
    "schema_version",
    "resolution_basis_ref",
    "primary_selection",
    "continuation_selection",
];
pub const RECIPE_APPLICATION_FIELD_NAMES_V1: [&str; 5] = [
    "schema_version",
    "resolution_basis_ref",
    "frontier_ref",
    "primary",
    "continuation",
];
pub const RECIPE_RETURN_OCCURRENCE_FIELD_NAMES_V1: [&str; 6] = [
    "schema_version",
    "resolution_basis_ref",
    "frontier_ref",
    "component",
    "outcome_tag",
    "return_reason_ref",
];
pub const JOB_RECIPE_ADMITTED_EDGES_V1: usize = 22;
pub const JOB_RECIPE_REFUSED_EDGES_V1: usize = 48;
pub const JOB_RECIPE_ADMITTED_APPLICATION_VECTORS_V1: usize = 66;
pub const JOB_RECIPE_REFUSED_APPLICATION_VECTORS_V1: usize = 144;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RecipeIdV1 {
    BoundedContinuation = 1,
    ConflictHandoff = 2,
    DesignRelay = 3,
    Fanout = 4,
    IntakeTriage = 5,
    Learning = 6,
    Setup = 7,
    Ship = 8,
    Synthesize = 9,
    Wayfinding = 10,
}

impl RecipeIdV1 {
    pub const ALL: [Self; 10] = [
        Self::BoundedContinuation,
        Self::ConflictHandoff,
        Self::DesignRelay,
        Self::Fanout,
        Self::IntakeTriage,
        Self::Learning,
        Self::Setup,
        Self::Ship,
        Self::Synthesize,
        Self::Wayfinding,
    ];

    pub const PRIMARY: [Self; 9] = [
        Self::ConflictHandoff,
        Self::DesignRelay,
        Self::Fanout,
        Self::IntakeTriage,
        Self::Learning,
        Self::Setup,
        Self::Ship,
        Self::Synthesize,
        Self::Wayfinding,
    ];

    pub const fn id(self) -> &'static str {
        RECIPE_IDS_V1[self as usize - 1]
    }

    pub const fn pascal(self) -> &'static str {
        match self {
            Self::BoundedContinuation => "BoundedContinuation",
            Self::ConflictHandoff => "ConflictHandoff",
            Self::DesignRelay => "DesignRelay",
            Self::Fanout => "Fanout",
            Self::IntakeTriage => "IntakeTriage",
            Self::Learning => "Learning",
            Self::Setup => "Setup",
            Self::Ship => "Ship",
            Self::Synthesize => "Synthesize",
            Self::Wayfinding => "Wayfinding",
        }
    }

    pub fn from_resource_ref(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|recipe| value == format!("candidate:orchestration:recipe:{}:v1", recipe.id()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum BoundedContinuationProfileV1 {
    Attended = 1,
    Unattended = 2,
}

impl BoundedContinuationProfileV1 {
    pub const ALL: [Self; 2] = [Self::Attended, Self::Unattended];

    pub const fn id(self) -> &'static str {
        BOUNDED_CONTINUATION_PROFILE_IDS_V1[self as usize - 1]
    }

    pub fn from_resource_ref(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| {
            let prefix = format!(
                "candidate:orchestration:bounded-continuation-profile:{}:v1@sha256:",
                profile.id()
            );
            value
                .strip_prefix(&prefix)
                .is_some_and(is_nonzero_lower_hex_digest)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeRoleV1 {
    Primary,
    ContinuationOverlay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeManifestV1 {
    pub recipe_id: String,
    pub semantic_version: [u64; 3],
    pub recipe_role: RecipeRoleV1,
    pub purpose_guidance_resource_ref: String,
    pub trigger_projection_reason_refs: Vec<String>,
    pub required_contract_refs: Vec<String>,
    pub restrictive_operation_filter_program_ref: String,
    pub phase_guidance_resource_refs: Vec<String>,
    pub required_projection_predicate_refs: Vec<String>,
    pub hard_stop_predicate_refs: Vec<String>,
    pub completion_predicate_refs: Vec<String>,
    pub return_reason_refs: Vec<String>,
    pub operating_limit_program_ref: String,
    pub allowed_continuation_profile_refs: Vec<String>,
}

impl RecipeManifestV1 {
    pub fn validate(&self) -> Result<(), RecipeLiteralError> {
        let recipe = RecipeIdV1::ALL
            .into_iter()
            .find(|candidate| candidate.id() == self.recipe_id)
            .ok_or(RecipeLiteralError::InvalidRecipeManifest)?;
        if self.semantic_version != [1, 0, 0]
            || self.purpose_guidance_resource_ref.is_empty()
            || self.trigger_projection_reason_refs.is_empty()
            || self.restrictive_operation_filter_program_ref.is_empty()
            || self.phase_guidance_resource_refs.len() != RECIPE_PHASES_V1.len()
            || self
                .phase_guidance_resource_refs
                .iter()
                .any(String::is_empty)
            || !all_unique(&self.phase_guidance_resource_refs)
            || self.completion_predicate_refs.is_empty()
            || self.operating_limit_program_ref.is_empty()
            || self.return_reason_refs != expected_return_reason_refs(recipe)
        {
            return Err(RecipeLiteralError::InvalidRecipeManifest);
        }
        match (recipe, self.recipe_role) {
            (RecipeIdV1::BoundedContinuation, RecipeRoleV1::ContinuationOverlay)
                if self.allowed_continuation_profile_refs
                    == BoundedContinuationProfileV1::ALL
                        .map(profile_resource_ref)
                        .to_vec() =>
            {
                Ok(())
            }
            (RecipeIdV1::BoundedContinuation, _) => Err(RecipeLiteralError::InvalidRecipeRole),
            (_, RecipeRoleV1::Primary) if self.allowed_continuation_profile_refs.is_empty() => {
                Ok(())
            }
            (_, _) => Err(RecipeLiteralError::InvalidRecipeRole),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactRecipeSelectionV1 {
    Absent,
    Primary {
        recipe_resource_ref: String,
        manifest_content_ref: String,
    },
    Continuation {
        recipe_resource_ref: String,
        manifest_content_ref: String,
        profile_resource_ref: String,
    },
}

impl ExactRecipeSelectionV1 {
    pub fn primary_recipe(&self) -> Option<RecipeIdV1> {
        match self {
            Self::Primary {
                recipe_resource_ref,
                ..
            } => RecipeIdV1::from_resource_ref(recipe_resource_ref),
            Self::Absent | Self::Continuation { .. } => None,
        }
    }

    pub fn continuation_profile(&self) -> Option<BoundedContinuationProfileV1> {
        match self {
            Self::Continuation {
                profile_resource_ref,
                ..
            } => BoundedContinuationProfileV1::from_resource_ref(profile_resource_ref),
            Self::Absent | Self::Primary { .. } => None,
        }
    }

    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    fn validate_primary(&self) -> Result<(), RecipeLiteralError> {
        match self {
            Self::Absent => Ok(()),
            Self::Primary {
                recipe_resource_ref,
                manifest_content_ref,
            } if RecipeIdV1::from_resource_ref(recipe_resource_ref)
                .is_some_and(|recipe| recipe != RecipeIdV1::BoundedContinuation)
                && is_sha256_ref(manifest_content_ref) =>
            {
                Ok(())
            }
            Self::Primary { .. } => Err(RecipeLiteralError::InvalidRecipeReference),
            Self::Continuation { .. } => Err(RecipeLiteralError::InvalidPrimarySelection),
        }
    }

    fn validate_continuation(&self) -> Result<(), RecipeLiteralError> {
        match self {
            Self::Absent => Ok(()),
            Self::Continuation {
                recipe_resource_ref,
                manifest_content_ref,
                profile_resource_ref,
            } if RecipeIdV1::from_resource_ref(recipe_resource_ref)
                == Some(RecipeIdV1::BoundedContinuation)
                && is_sha256_ref(manifest_content_ref)
                && BoundedContinuationProfileV1::from_resource_ref(profile_resource_ref)
                    .is_some() =>
            {
                Ok(())
            }
            Self::Continuation { .. } => Err(RecipeLiteralError::InvalidRecipeReference),
            Self::Primary { .. } => Err(RecipeLiteralError::InvalidContinuationSelection),
        }
    }

    fn encode(&self, output: &mut Vec<u8>) {
        match self {
            Self::Absent => {
                encode_array_len(1, output);
                encode_u64(1, output);
            }
            Self::Primary {
                recipe_resource_ref,
                manifest_content_ref,
            } => {
                encode_array_len(3, output);
                encode_u64(2, output);
                encode_text(recipe_resource_ref, output);
                encode_text(manifest_content_ref, output);
            }
            Self::Continuation {
                recipe_resource_ref,
                manifest_content_ref,
                profile_resource_ref,
            } => {
                encode_array_len(4, output);
                encode_u64(3, output);
                encode_text(recipe_resource_ref, output);
                encode_text(manifest_content_ref, output);
                encode_text(profile_resource_ref, output);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeSelectionRequestV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub primary_selection: ExactRecipeSelectionV1,
    pub continuation_selection: ExactRecipeSelectionV1,
}

impl RecipeSelectionRequestV1 {
    pub fn validate(&self) -> Result<(), RecipeLiteralError> {
        if self.schema_version != 1 || self.resolution_basis_ref.is_empty() {
            return Err(RecipeLiteralError::InvalidSelectionRequest);
        }
        self.primary_selection.validate_primary()?;
        self.continuation_selection.validate_continuation()
    }

    pub fn semantic_hash(&self) -> Result<[u8; 32], RecipeLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(4, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_text(&self.resolution_basis_ref, &mut value);
        self.primary_selection.encode(&mut value);
        self.continuation_selection.encode(&mut value);
        Ok(domain_hash(
            "maestro.vnext.recipe-selection-request.v1",
            &value,
        ))
    }

    pub fn seal(
        self,
        frontier_ref: impl Into<String>,
    ) -> Result<RecipeApplicationV1, RecipeLiteralError> {
        self.validate()?;
        let frontier_ref = frontier_ref.into();
        if frontier_ref.is_empty() {
            return Err(RecipeLiteralError::MissingFrontierForApplication);
        }
        let application = RecipeApplicationV1 {
            schema_version: self.schema_version,
            resolution_basis_ref: self.resolution_basis_ref,
            frontier_ref,
            primary: self.primary_selection,
            continuation: self.continuation_selection,
        };
        application.validate()?;
        Ok(application)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeApplicationV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub frontier_ref: String,
    pub primary: ExactRecipeSelectionV1,
    pub continuation: ExactRecipeSelectionV1,
}

impl RecipeApplicationV1 {
    pub fn validate(&self) -> Result<(), RecipeLiteralError> {
        RecipeSelectionRequestV1 {
            schema_version: self.schema_version,
            resolution_basis_ref: self.resolution_basis_ref.clone(),
            primary_selection: self.primary.clone(),
            continuation_selection: self.continuation.clone(),
        }
        .validate()?;
        if self.frontier_ref.is_empty() {
            return Err(RecipeLiteralError::MissingFrontierForApplication);
        }
        Ok(())
    }

    pub fn selection_request(&self) -> RecipeSelectionRequestV1 {
        RecipeSelectionRequestV1 {
            schema_version: self.schema_version,
            resolution_basis_ref: self.resolution_basis_ref.clone(),
            primary_selection: self.primary.clone(),
            continuation_selection: self.continuation.clone(),
        }
    }

    pub fn semantic_hash(&self) -> Result<[u8; 32], RecipeLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(5, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_text(&self.resolution_basis_ref, &mut value);
        encode_text(&self.frontier_ref, &mut value);
        self.primary.encode(&mut value);
        self.continuation.encode(&mut value);
        Ok(domain_hash("maestro.vnext.recipe-application.v1", &value))
    }

    pub fn component_count(&self) -> usize {
        usize::from(!self.primary.is_absent()) + usize::from(!self.continuation.is_absent())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RecipeReturnReasonV1 {
    BoundedContinuationNotApplicable = 1,
    BoundedContinuationRestrictiveAdvice = 2,
    BoundedContinuationHardStop = 3,
    ConflictHandoffNotApplicable = 4,
    ConflictHandoffRestrictiveAdvice = 5,
    ConflictHandoffHardStop = 6,
    DesignRelayNotApplicable = 7,
    DesignRelayRestrictiveAdvice = 8,
    DesignRelayHardStop = 9,
    FanoutNotApplicable = 10,
    FanoutRestrictiveAdvice = 11,
    FanoutHardStop = 12,
    IntakeTriageNotApplicable = 13,
    IntakeTriageRestrictiveAdvice = 14,
    IntakeTriageHardStop = 15,
    LearningNotApplicable = 16,
    LearningRestrictiveAdvice = 17,
    LearningHardStop = 18,
    SetupNotApplicable = 19,
    SetupRestrictiveAdvice = 20,
    SetupHardStop = 21,
    ShipNotApplicable = 22,
    ShipRestrictiveAdvice = 23,
    ShipHardStop = 24,
    SynthesizeNotApplicable = 25,
    SynthesizeRestrictiveAdvice = 26,
    SynthesizeHardStop = 27,
    WayfindingNotApplicable = 28,
    WayfindingRestrictiveAdvice = 29,
    WayfindingHardStop = 30,
}

pub const RECIPE_RETURN_REASON_COUNT_V1: usize = 30;
pub const RECIPE_RETURN_REASON_MANIFEST_SUBSETS_V1: usize = 10;
pub const RECIPE_RETURN_REASON_MEMBERSHIP_POSITIVE_V1: usize = 30;
pub const RECIPE_RETURN_REASON_MEMBERSHIP_NEGATIVE_V1: usize = 270;
pub const RECIPE_RETURN_REASON_COMPATIBILITY_POSITIVE_V1: usize = 30;
pub const RECIPE_RETURN_REASON_COMPATIBILITY_NEGATIVE_V1: usize = 870;
pub const RECIPE_RETURN_OCCURRENCE_VECTORS_V1: usize = 196;

impl RecipeReturnReasonV1 {
    pub const ALL: [Self; 30] = [
        Self::BoundedContinuationNotApplicable,
        Self::BoundedContinuationRestrictiveAdvice,
        Self::BoundedContinuationHardStop,
        Self::ConflictHandoffNotApplicable,
        Self::ConflictHandoffRestrictiveAdvice,
        Self::ConflictHandoffHardStop,
        Self::DesignRelayNotApplicable,
        Self::DesignRelayRestrictiveAdvice,
        Self::DesignRelayHardStop,
        Self::FanoutNotApplicable,
        Self::FanoutRestrictiveAdvice,
        Self::FanoutHardStop,
        Self::IntakeTriageNotApplicable,
        Self::IntakeTriageRestrictiveAdvice,
        Self::IntakeTriageHardStop,
        Self::LearningNotApplicable,
        Self::LearningRestrictiveAdvice,
        Self::LearningHardStop,
        Self::SetupNotApplicable,
        Self::SetupRestrictiveAdvice,
        Self::SetupHardStop,
        Self::ShipNotApplicable,
        Self::ShipRestrictiveAdvice,
        Self::ShipHardStop,
        Self::SynthesizeNotApplicable,
        Self::SynthesizeRestrictiveAdvice,
        Self::SynthesizeHardStop,
        Self::WayfindingNotApplicable,
        Self::WayfindingRestrictiveAdvice,
        Self::WayfindingHardStop,
    ];

    pub const fn recipe(self) -> RecipeIdV1 {
        RecipeIdV1::ALL[(self as usize - 1) / 3]
    }

    pub const fn outcome(self) -> RecipeComponentOutcomeTagV1 {
        match (self as u8 - 1) % 3 {
            0 => RecipeComponentOutcomeTagV1::NotApplicable,
            1 => RecipeComponentOutcomeTagV1::RestrictiveAdvice,
            _ => RecipeComponentOutcomeTagV1::HardStop,
        }
    }

    pub fn name(self) -> String {
        format!("{}{}", self.recipe().pascal(), self.outcome().name())
    }

    pub fn resource_ref(self) -> String {
        format!(
            "candidate:orchestration:recipe-return-reason:{}:v1",
            self.name()
        )
    }

    pub const fn for_pair(recipe: RecipeIdV1, outcome: RecipeComponentOutcomeTagV1) -> Self {
        Self::ALL[(recipe as usize - 1) * 3 + outcome as usize - 1]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RecipeComponentOutcomeTagV1 {
    NotApplicable = 1,
    RestrictiveAdvice = 2,
    HardStop = 3,
}

impl RecipeComponentOutcomeTagV1 {
    pub const ALL: [Self; 3] = [Self::NotApplicable, Self::RestrictiveAdvice, Self::HardStop];

    pub const fn name(self) -> &'static str {
        match self {
            Self::NotApplicable => "NotApplicable",
            Self::RestrictiveAdvice => "RestrictiveAdvice",
            Self::HardStop => "HardStop",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeReturnComponentV1 {
    Primary {
        recipe_resource_ref: String,
        manifest_content_ref: String,
    },
    Continuation {
        recipe_resource_ref: String,
        manifest_content_ref: String,
        profile_resource_ref: String,
    },
}

impl RecipeReturnComponentV1 {
    pub fn recipe(&self) -> Option<RecipeIdV1> {
        match self {
            Self::Primary {
                recipe_resource_ref,
                ..
            }
            | Self::Continuation {
                recipe_resource_ref,
                ..
            } => RecipeIdV1::from_resource_ref(recipe_resource_ref),
        }
    }

    pub fn slot(&self) -> RecipeComponentSlotV1 {
        match self {
            Self::Primary { .. } => RecipeComponentSlotV1::Primary,
            Self::Continuation { .. } => RecipeComponentSlotV1::Continuation,
        }
    }

    pub fn matches_selection(&self, selection: &ExactRecipeSelectionV1) -> bool {
        match (self, selection) {
            (
                Self::Primary {
                    recipe_resource_ref,
                    manifest_content_ref,
                },
                ExactRecipeSelectionV1::Primary {
                    recipe_resource_ref: selected_recipe,
                    manifest_content_ref: selected_manifest,
                },
            ) => {
                recipe_resource_ref == selected_recipe && manifest_content_ref == selected_manifest
            }
            (
                Self::Continuation {
                    recipe_resource_ref,
                    manifest_content_ref,
                    profile_resource_ref,
                },
                ExactRecipeSelectionV1::Continuation {
                    recipe_resource_ref: selected_recipe,
                    manifest_content_ref: selected_manifest,
                    profile_resource_ref: selected_profile,
                },
            ) => {
                recipe_resource_ref == selected_recipe
                    && manifest_content_ref == selected_manifest
                    && profile_resource_ref == selected_profile
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RecipeComponentSlotV1 {
    Primary = 1,
    Continuation = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeReturnReasonRefV1 {
    pub recipe_return_reason_resource_ref: String,
    pub reason: RecipeReturnReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeReturnOccurrenceV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub frontier_ref: String,
    pub component: RecipeReturnComponentV1,
    pub outcome_tag: RecipeComponentOutcomeTagV1,
    pub return_reason_ref: RecipeReturnReasonRefV1,
}

impl RecipeReturnOccurrenceV1 {
    pub fn validate(&self) -> Result<(), RecipeLiteralError> {
        let recipe = self
            .component
            .recipe()
            .ok_or(RecipeLiteralError::InvalidReturnOccurrence)?;
        let expected = RecipeReturnReasonV1::for_pair(recipe, self.outcome_tag);
        if self.schema_version != 1
            || self.resolution_basis_ref.is_empty()
            || self.frontier_ref.is_empty()
            || self.return_reason_ref.reason != expected
            || self.return_reason_ref.recipe_return_reason_resource_ref != expected.resource_ref()
        {
            return Err(RecipeLiteralError::InvalidReturnOccurrence);
        }
        match &self.component {
            RecipeReturnComponentV1::Primary {
                manifest_content_ref,
                ..
            } if recipe != RecipeIdV1::BoundedContinuation
                && is_sha256_ref(manifest_content_ref) =>
            {
                Ok(())
            }
            RecipeReturnComponentV1::Continuation {
                manifest_content_ref,
                profile_resource_ref,
                ..
            } if recipe == RecipeIdV1::BoundedContinuation
                && is_sha256_ref(manifest_content_ref)
                && BoundedContinuationProfileV1::from_resource_ref(profile_resource_ref)
                    .is_some() =>
            {
                Ok(())
            }
            _ => Err(RecipeLiteralError::InvalidReturnOccurrence),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeComponentEvaluationV1 {
    NotApplicable(RecipeReturnOccurrenceV1),
    RestrictiveAdvice {
        recipe_advice_ref: String,
        occurrence: RecipeReturnOccurrenceV1,
    },
    HardStop {
        recipe_hard_stop_ref: String,
        occurrence: RecipeReturnOccurrenceV1,
    },
}

impl RecipeComponentEvaluationV1 {
    pub fn validate(&self) -> Result<(), RecipeLiteralError> {
        let (expected, occurrence, payload_ref) = match self {
            Self::NotApplicable(occurrence) => {
                (RecipeComponentOutcomeTagV1::NotApplicable, occurrence, None)
            }
            Self::RestrictiveAdvice {
                recipe_advice_ref,
                occurrence,
            } => (
                RecipeComponentOutcomeTagV1::RestrictiveAdvice,
                occurrence,
                Some(recipe_advice_ref),
            ),
            Self::HardStop {
                recipe_hard_stop_ref,
                occurrence,
            } => (
                RecipeComponentOutcomeTagV1::HardStop,
                occurrence,
                Some(recipe_hard_stop_ref),
            ),
        };
        occurrence.validate()?;
        if occurrence.outcome_tag != expected || payload_ref.is_some_and(String::is_empty) {
            return Err(RecipeLiteralError::InvalidReturnOccurrence);
        }
        Ok(())
    }
}

pub fn recipe_resource_ref(recipe: RecipeIdV1) -> String {
    format!("candidate:orchestration:recipe:{}:v1", recipe.id())
}

pub fn profile_resource_ref(profile: BoundedContinuationProfileV1) -> String {
    format!(
        "candidate:orchestration:bounded-continuation-profile:{}:v1",
        profile.id()
    )
}

fn expected_return_reason_refs(recipe: RecipeIdV1) -> Vec<String> {
    RecipeComponentOutcomeTagV1::ALL
        .map(|outcome| RecipeReturnReasonV1::for_pair(recipe, outcome).resource_ref())
        .to_vec()
}

fn is_sha256_ref(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(is_nonzero_lower_hex_digest)
}

fn is_nonzero_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn all_unique(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn domain_hash(domain: &str, canonical_value: &[u8]) -> [u8; 32] {
    let mut encoded = Vec::new();
    encode_array_len(2, &mut encoded);
    encode_text(domain, &mut encoded);
    encoded.extend_from_slice(canonical_value);
    Sha256::digest(encoded).into()
}

fn encode_array_len(value: usize, output: &mut Vec<u8>) {
    encode_head(4, value as u64, output);
}

fn encode_u64(value: u64, output: &mut Vec<u8>) {
    encode_head(0, value, output);
}

fn encode_text(value: &str, output: &mut Vec<u8>) {
    encode_head(3, value.len() as u64, output);
    output.extend_from_slice(value.as_bytes());
}

fn encode_head(major: u8, value: u64, output: &mut Vec<u8>) {
    if value < 24 {
        output.push((major << 5) | value as u8);
    } else if value <= u8::MAX.into() {
        output.extend_from_slice(&[(major << 5) | 24, value as u8]);
    } else if value <= u16::MAX.into() {
        output.push((major << 5) | 25);
        output.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= u32::MAX.into() {
        output.push((major << 5) | 26);
        output.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        output.push((major << 5) | 27);
        output.extend_from_slice(&value.to_be_bytes());
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RecipeLiteralError {
    #[error(
        "RecipeManifestV1 must preserve the exact catalog, typed refs, six phases, and per-Recipe reason subset"
    )]
    InvalidRecipeManifest,
    #[error("Recipe role does not match the exact primary or bounded-continuation catalog role")]
    InvalidRecipeRole,
    #[error("RecipeSelectionRequestV1 is invalid before Frontier sealing")]
    InvalidSelectionRequest,
    #[error("a primary Recipe selection cannot use a continuation payload")]
    InvalidPrimarySelection,
    #[error("a continuation selection cannot use a primary payload")]
    InvalidContinuationSelection,
    #[error(
        "a selected Recipe or profile reference is absent, stale-shaped, or outside the exact catalog"
    )]
    InvalidRecipeReference,
    #[error("RecipeApplicationV1 requires one exact Frontier only at sealing")]
    MissingFrontierForApplication,
    #[error("RecipeReturnOccurrenceV1 is not the exact Recipe/outcome/resource bijection")]
    InvalidReturnOccurrence,
}
