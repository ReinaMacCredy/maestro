use std::fmt::Write;

#[cfg(test)]
use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::orchestration::literals::{
    BoundedContinuationProfileV1, RecipeIdV1, RecipeLiteralError, RecipeManifestV1, RecipeRoleV1,
    profile_resource_ref, recipe_resource_ref,
};

const CATALOG_BYTES: &str =
    include_str!("../../../../embedded/vnext/orchestration/recipe-catalog.v1.json");
const ATTENDED_PROFILE_BYTES: &str = include_str!(
    "../../../../embedded/vnext/orchestration/profiles/bounded-continuation/attended.v1.json"
);
const UNATTENDED_PROFILE_BYTES: &str = include_str!(
    "../../../../embedded/vnext/orchestration/profiles/bounded-continuation/unattended.v1.json"
);
const MANIFEST_BYTES: [&str; 10] = [
    include_str!(
        "../../../../embedded/vnext/orchestration/recipes/bounded-continuation/manifest.v1.json"
    ),
    include_str!(
        "../../../../embedded/vnext/orchestration/recipes/conflict-handoff/manifest.v1.json"
    ),
    include_str!("../../../../embedded/vnext/orchestration/recipes/design-relay/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/fanout/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/intake-triage/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/learning/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/setup/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/ship/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/synthesize/manifest.v1.json"),
    include_str!("../../../../embedded/vnext/orchestration/recipes/wayfinding/manifest.v1.json"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenRecipeEntryV1 {
    pub(crate) recipe: RecipeIdV1,
    pub(crate) recipe_resource_ref: String,
    pub(crate) manifest_content_ref: String,
    pub(crate) manifest: RecipeManifestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenRecipeProfileV1 {
    pub(crate) profile: BoundedContinuationProfileV1,
    pub(crate) profile_resource_ref: String,
    pub(crate) ambiguity_or_missing_authority: ProfileAmbiguityDispositionV1,
    pub(crate) may_only_tighten: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProfileAmbiguityDispositionV1 {
    ReturnForMaterialChoice,
    HardStop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenRecipeCatalogV1 {
    pub(crate) catalog_content_ref: String,
    pub(crate) recipes: Vec<FrozenRecipeEntryV1>,
    pub(crate) profiles: Vec<FrozenRecipeProfileV1>,
}

impl FrozenRecipeCatalogV1 {
    pub(crate) fn load() -> Result<Self, FrozenRecipeCatalogErrorV1> {
        let source: CatalogSourceV1 = serde_json::from_str(CATALOG_BYTES)?;
        if source.schema != "maestro.vnext.recipe-catalog-source.v1"
            || !source.candidate_only
            || source.runtime_activation
            || source.runtime_registration
            || source.recipes.len() != RecipeIdV1::ALL.len()
            || source.bounded_continuation_profiles.len() != BoundedContinuationProfileV1::ALL.len()
        {
            return Err(FrozenRecipeCatalogErrorV1::CatalogMismatch);
        }
        let mut recipes = Vec::with_capacity(RecipeIdV1::ALL.len());
        for ((source_row, recipe), manifest_bytes) in source
            .recipes
            .iter()
            .zip(RecipeIdV1::ALL)
            .zip(MANIFEST_BYTES)
        {
            let expected_role = if recipe == RecipeIdV1::BoundedContinuation {
                "ContinuationOverlay"
            } else {
                "Primary"
            };
            if source_row.id != recipe.id()
                || source_row.role != expected_role
                || source_row.manifest_path != format!("recipes/{}/manifest.v1.json", recipe.id())
            {
                return Err(FrozenRecipeCatalogErrorV1::CatalogMismatch);
            }
            let manifest_source: ManifestSourceV1 = serde_json::from_str(manifest_bytes)?;
            let manifest = manifest_source.into_manifest()?;
            manifest.validate()?;
            if manifest.recipe_id != recipe.id() {
                return Err(FrozenRecipeCatalogErrorV1::ManifestMismatch);
            }
            recipes.push(FrozenRecipeEntryV1 {
                recipe,
                recipe_resource_ref: recipe_resource_ref(recipe),
                manifest_content_ref: sha256_ref(manifest_bytes.as_bytes()),
                manifest,
            });
        }
        let profile_bytes = [ATTENDED_PROFILE_BYTES, UNATTENDED_PROFILE_BYTES];
        let mut profiles = Vec::with_capacity(2);
        for ((source_row, profile), bytes) in source
            .bounded_continuation_profiles
            .iter()
            .zip(BoundedContinuationProfileV1::ALL)
            .zip(profile_bytes)
        {
            if source_row.id != profile.id()
                || source_row.resource_path
                    != format!("profiles/bounded-continuation/{}.v1.json", profile.id())
                || source_row.may_only_tighten != exact_tightening_axes()
            {
                return Err(FrozenRecipeCatalogErrorV1::ProfileMismatch);
            }
            let profile_source: ProfileSourceV1 = serde_json::from_str(bytes)?;
            let parsed = profile_source.validate(profile)?;
            profiles.push(FrozenRecipeProfileV1 {
                profile,
                profile_resource_ref: format!(
                    "{}@{}",
                    profile_resource_ref(profile),
                    sha256_ref(bytes.as_bytes())
                ),
                ambiguity_or_missing_authority: parsed,
                may_only_tighten: source_row.may_only_tighten.clone(),
            });
        }
        Ok(Self {
            catalog_content_ref: sha256_ref(CATALOG_BYTES.as_bytes()),
            recipes,
            profiles,
        })
    }

    pub(crate) fn recipe(&self, recipe: RecipeIdV1) -> Option<&FrozenRecipeEntryV1> {
        self.recipes.get(recipe as usize - 1)
    }

    pub(crate) fn profile(
        &self,
        profile: BoundedContinuationProfileV1,
    ) -> Option<&FrozenRecipeProfileV1> {
        self.profiles.get(profile as usize - 1)
    }

    pub(crate) fn validates_primary(&self, recipe_resource: &str, manifest_content: &str) -> bool {
        RecipeIdV1::from_resource_ref(recipe_resource)
            .filter(|recipe| *recipe != RecipeIdV1::BoundedContinuation)
            .and_then(|recipe| self.recipe(recipe))
            .is_some_and(|entry| {
                entry.recipe_resource_ref == recipe_resource
                    && entry.manifest_content_ref == manifest_content
            })
    }

    pub(crate) fn validates_continuation(
        &self,
        recipe_resource: &str,
        manifest_content: &str,
        profile_resource: &str,
    ) -> bool {
        let Some(entry) = self.recipe(RecipeIdV1::BoundedContinuation) else {
            return false;
        };
        let Some(profile) = BoundedContinuationProfileV1::from_resource_ref(profile_resource)
        else {
            return false;
        };
        entry.recipe_resource_ref == recipe_resource
            && entry.manifest_content_ref == manifest_content
            && self
                .profile(profile)
                .is_some_and(|entry| entry.profile_resource_ref == profile_resource)
    }
}

#[derive(Deserialize)]
struct CatalogSourceV1 {
    schema: String,
    candidate_only: bool,
    runtime_activation: bool,
    runtime_registration: bool,
    recipes: Vec<CatalogRecipeSourceV1>,
    bounded_continuation_profiles: Vec<CatalogProfileSourceV1>,
}

#[derive(Deserialize)]
struct CatalogRecipeSourceV1 {
    id: String,
    role: String,
    manifest_path: String,
}

#[derive(Deserialize)]
struct CatalogProfileSourceV1 {
    id: String,
    resource_path: String,
    may_only_tighten: Vec<String>,
}

#[derive(Deserialize)]
struct ManifestSourceV1 {
    recipe_id: String,
    semantic_version: [u64; 3],
    recipe_role: String,
    purpose_guidance_resource_ref: String,
    trigger_projection_reason_refs: Vec<String>,
    required_contract_refs: Vec<String>,
    restrictive_operation_filter_program_ref: String,
    phase_guidance_resource_refs: Vec<String>,
    required_projection_predicate_refs: Vec<String>,
    hard_stop_predicate_refs: Vec<String>,
    completion_predicate_refs: Vec<String>,
    return_reason_refs: Vec<String>,
    operating_limit_program_ref: String,
    allowed_continuation_profile_refs: Vec<String>,
}

impl ManifestSourceV1 {
    fn into_manifest(self) -> Result<RecipeManifestV1, FrozenRecipeCatalogErrorV1> {
        let recipe_role = match self.recipe_role.as_str() {
            "Primary" => RecipeRoleV1::Primary,
            "ContinuationOverlay" => RecipeRoleV1::ContinuationOverlay,
            _ => return Err(FrozenRecipeCatalogErrorV1::ManifestMismatch),
        };
        Ok(RecipeManifestV1 {
            recipe_id: self.recipe_id,
            semantic_version: self.semantic_version,
            recipe_role,
            purpose_guidance_resource_ref: self.purpose_guidance_resource_ref,
            trigger_projection_reason_refs: self.trigger_projection_reason_refs,
            required_contract_refs: self.required_contract_refs,
            restrictive_operation_filter_program_ref: self.restrictive_operation_filter_program_ref,
            phase_guidance_resource_refs: self.phase_guidance_resource_refs,
            required_projection_predicate_refs: self.required_projection_predicate_refs,
            hard_stop_predicate_refs: self.hard_stop_predicate_refs,
            completion_predicate_refs: self.completion_predicate_refs,
            return_reason_refs: self.return_reason_refs,
            operating_limit_program_ref: self.operating_limit_program_ref,
            allowed_continuation_profile_refs: self.allowed_continuation_profile_refs,
        })
    }
}

#[derive(Deserialize)]
struct ProfileSourceV1 {
    schema: String,
    candidate_only: bool,
    runtime_activation: bool,
    runtime_registration: bool,
    profile_id: String,
    recipe_id: String,
    ambiguity_or_missing_authority: String,
    may_only_tighten: Vec<String>,
    forbidden_semantics: Vec<String>,
}

impl ProfileSourceV1 {
    fn validate(
        self,
        profile: BoundedContinuationProfileV1,
    ) -> Result<ProfileAmbiguityDispositionV1, FrozenRecipeCatalogErrorV1> {
        let exact_forbidden = [
            "Recommendation",
            "OperationSelection",
            "Authority",
            "Lifecycle",
            "Mutation",
            "RetryRight",
            "Cursor",
            "WorkerRuntime",
        ]
        .map(str::to_owned)
        .to_vec();
        if self.schema != "maestro.vnext.bounded-continuation-profile.v1"
            || !self.candidate_only
            || self.runtime_activation
            || self.runtime_registration
            || self.profile_id != profile.id()
            || self.recipe_id != RecipeIdV1::BoundedContinuation.id()
            || self.may_only_tighten != exact_tightening_axes()
            || self.forbidden_semantics != exact_forbidden
        {
            return Err(FrozenRecipeCatalogErrorV1::ProfileMismatch);
        }
        match (profile, self.ambiguity_or_missing_authority.as_str()) {
            (BoundedContinuationProfileV1::Attended, "ReturnForMaterialChoice") => {
                Ok(ProfileAmbiguityDispositionV1::ReturnForMaterialChoice)
            }
            (BoundedContinuationProfileV1::Unattended, "HardStop") => {
                Ok(ProfileAmbiguityDispositionV1::HardStop)
            }
            _ => Err(FrozenRecipeCatalogErrorV1::ProfileMismatch),
        }
    }
}

fn exact_tightening_axes() -> Vec<String> {
    [
        "cadence",
        "attempts",
        "time",
        "cost",
        "subagent_count",
        "connector_permissions",
        "denylist",
        "hard_stops",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn sha256_ref(bytes: &[u8]) -> String {
    let mut value = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        write!(&mut value, "{byte:02x}")
            .expect("invariant: hexadecimal rendering into String is infallible");
    }
    value
}

#[derive(Debug, Error)]
pub(crate) enum FrozenRecipeCatalogErrorV1 {
    #[error("embedded Recipe catalog is not the exact frozen ten-plus-two closure")]
    CatalogMismatch,
    #[error("embedded Recipe Manifest differs from its frozen catalog row")]
    ManifestMismatch,
    #[error("embedded bounded-continuation profile differs from its frozen row")]
    ProfileMismatch,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Literal(#[from] RecipeLiteralError),
}

#[cfg(test)]
pub(crate) fn frozen_source_count() -> usize {
    let unique = MANIFEST_BYTES
        .iter()
        .copied()
        .chain([
            ATTENDED_PROFILE_BYTES,
            UNATTENDED_PROFILE_BYTES,
            CATALOG_BYTES,
        ])
        .collect::<BTreeSet<_>>();
    unique.len()
}
