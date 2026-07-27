use crate::domain::contract::component_kind::ContractComponentKindV1;
use crate::domain::contract::finalization::FinalizationInputKindV1;
use crate::domain::identity::{
    ConstraintExprV1, FieldDescriptorV1, SchemaClosureV1, SchemaDescriptorV1, SchemaError,
    SchemaIdV1, Stage0ProofManifestIdV1, TypeExprV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

pub fn candidate_root_schema_closure_v1() -> Result<SchemaClosureV1, SchemaError> {
    let mut descriptors = vec![record_schema(
        "NormativeInputsDecisionMaterializationV1",
        vec![
            ("version", TypeExprV1::Unsigned),
            ("materialization_id", TypeExprV1::ExactBytes(32)),
            (
                "decision_sources",
                TypeExprV1::OrderedList(Box::new(TypeExprV1::Tuple(vec![
                    TypeExprV1::AsciiText,
                    TypeExprV1::ExactBytes(32),
                ]))),
            ),
        ],
    )?];
    for kind in ContractComponentKindV1::ALL {
        if kind == ContractComponentKindV1::NormativeInputs {
            continue;
        }
        let mut fields = vec![("version", TypeExprV1::Unsigned)];
        fields.extend(
            facet_fields(kind)
                .iter()
                .map(|(name, kind)| (*name, field_type(*kind))),
        );
        descriptors.push(record_schema(
            &format!("CandidateRootFacet{}V1", kind.tag()),
            fields,
        )?);
    }
    for kind in FinalizationInputKindV1::ALL {
        let mut fields = vec![
            ("version", TypeExprV1::Unsigned),
            ("input_kind", TypeExprV1::Unsigned),
            ("design_revision_id", TypeExprV1::ExactBytes(32)),
            ("decision_closure_id", TypeExprV1::ExactBytes(32)),
            ("candidate_contract_root_id", TypeExprV1::ExactBytes(32)),
            (
                "owner_facet_component_ids",
                TypeExprV1::OrderedList(Box::new(TypeExprV1::ExactBytes(32))),
            ),
        ];
        if kind == FinalizationInputKindV1::StageProofMatrix {
            fields.extend([
                ("stage0_proof_manifest_id", TypeExprV1::ExactBytes(32)),
                (
                    "stage0_proof_manifest_artifact_sha256",
                    TypeExprV1::ExactBytes(32),
                ),
                ("stage0_proof_gate_count", TypeExprV1::Unsigned),
            ]);
        }
        descriptors.push(record_schema(kind.schema_name(), fields)?);
    }
    SchemaClosureV1::new(descriptors)
}

pub fn normative_inputs_schema_id_v1(
    schema_closure: &SchemaClosureV1,
) -> Result<SchemaIdV1, SchemaError> {
    schema_id(schema_closure, "NormativeInputsDecisionMaterializationV1")
}

pub fn facet_schema_id_v1(
    schema_closure: &SchemaClosureV1,
    kind: ContractComponentKindV1,
) -> Result<SchemaIdV1, SchemaError> {
    schema_id(
        schema_closure,
        &format!("CandidateRootFacet{}V1", kind.tag()),
    )
}

pub fn finalization_input_schema_id_v1(
    schema_closure: &SchemaClosureV1,
    kind: FinalizationInputKindV1,
) -> Result<SchemaIdV1, SchemaError> {
    schema_id(schema_closure, kind.schema_name())
}

pub fn fixture_facet_value_v1(
    kind: ContractComponentKindV1,
    commitment: [u8; 32],
    descriptor_ids: Vec<[u8; 32]>,
) -> CborValue {
    let mut fields = vec![CborValue::Unsigned(1)];
    for (_, field_kind) in facet_fields(kind) {
        fields.push(match field_kind {
            FacetFieldTypeV1::Bytes => CborValue::Bytes(commitment.to_vec()),
            FacetFieldTypeV1::BytesList => CborValue::Array(
                descriptor_ids
                    .iter()
                    .map(|identifier| CborValue::Bytes(identifier.to_vec()))
                    .collect(),
            ),
            FacetFieldTypeV1::Unsigned => CborValue::Unsigned(
                u64::try_from(descriptor_ids.len())
                    .expect("invariant: fixture descriptor count fits u64"),
            ),
        });
    }
    CborValue::Array(fields)
}

pub fn stage_proof_matrix_facet_value_v1(
    manifest_id: Stage0ProofManifestIdV1,
    artifact_sha256: [u8; 32],
    gate_count: u64,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(artifact_sha256.to_vec()),
        CborValue::Unsigned(gate_count),
    ])
}

pub fn finalization_facet_kinds_v1(
    kind: FinalizationInputKindV1,
) -> &'static [ContractComponentKindV1] {
    use ContractComponentKindV1 as Component;
    use FinalizationInputKindV1 as Input;

    match kind {
        Input::ClosureRequirement => &[
            Component::IntendedOutcome,
            Component::AcceptanceBoundary,
            Component::MaterialScope,
            Component::AffectedSurfaces,
            Component::NonGoals,
        ],
        Input::DeterministicSynthesis => &[
            Component::StepDefinitions,
            Component::StepGraphSnapshot,
            Component::GateSnapshot,
            Component::PolicyProfileProvenance,
        ],
        Input::ScopeAndExclusions => &[
            Component::MaterialScope,
            Component::AffectedSurfaces,
            Component::NonGoals,
        ],
        Input::CapabilityCensusAndJourneys => &[
            Component::CapabilityCensus,
            Component::LiteralManifestClosure,
        ],
        Input::MigrationRollbackRemoval => &[Component::MigrationRollbackRemoval],
        Input::StageProofMatrix => &[Component::StageProofMatrix],
        Input::ReviewEvidence => &[
            Component::PolicyProfileProvenance,
            Component::StageProofMatrix,
        ],
        Input::EdgeSweepEvidence => &[Component::ExternalTargets, Component::OperatingConstraints],
        Input::RiskRecovery => &[
            Component::ResourceLimits,
            Component::ExternalTargets,
            Component::OperatingConstraints,
            Component::MigrationRollbackRemoval,
        ],
        Input::FreshnessReferences => &[
            Component::LiteralSchemaClosure,
            Component::LiteralManifestClosure,
            Component::ResourceClosure,
            Component::BundleClosure,
            Component::ReleaseResourceCensus,
            Component::ReleaseClosure,
        ],
        Input::CanonicalizationPolicy => &[
            Component::LiteralSchemaClosure,
            Component::LiteralManifestClosure,
            Component::StageProofMatrix,
        ],
    }
}

#[derive(Clone, Copy)]
enum FacetFieldTypeV1 {
    Bytes,
    BytesList,
    Unsigned,
}

fn facet_fields(kind: ContractComponentKindV1) -> &'static [(&'static str, FacetFieldTypeV1)] {
    use ContractComponentKindV1 as Kind;
    use FacetFieldTypeV1::{Bytes, BytesList, Unsigned};

    match kind {
        Kind::IntendedOutcome => &[("design_sha256", Bytes)],
        Kind::AcceptanceBoundary => &[("acceptance_card_sha256", Bytes)],
        Kind::MaterialScope => &[("design_sha256", Bytes), ("acceptance_card_sha256", Bytes)],
        Kind::AffectedSurfaces | Kind::NonGoals => &[("design_sha256", Bytes)],
        Kind::StepDefinitions
        | Kind::StepGraphSnapshot
        | Kind::GateSnapshot
        | Kind::PolicyProfileProvenance
        | Kind::PublicationAuthorityRequirement
        | Kind::CompletionAuthorityRequirement => &[("decision_closure_id", Bytes)],
        Kind::ResourceLimits => &[("acceptance_card_sha256", Bytes)],
        Kind::ExternalTargets => &[
            ("public_identity_closure_id", Bytes),
            ("resource_release_id", Bytes),
        ],
        Kind::OperatingConstraints => {
            &[("design_sha256", Bytes), ("acceptance_card_sha256", Bytes)]
        }
        Kind::CapabilityCensus => &[
            ("public_identity_closure_id", Bytes),
            ("effect_home_id", Bytes),
        ],
        Kind::LiteralSchemaClosure => &[
            ("public_schema_descriptor_ids", BytesList),
            ("submission_claim_set_schema_id", Bytes),
            ("submission_claim_set_artifact_sha256", Bytes),
        ],
        Kind::LiteralManifestClosure => &[
            ("public_manifest_id", Bytes),
            ("public_identity_closure_id", Bytes),
            ("effect_home_id", Bytes),
        ],
        Kind::ResourceClosure => &[
            ("public_resource_input_id", Bytes),
            ("resource_release_id", Bytes),
        ],
        Kind::BundleClosure | Kind::ReleaseResourceCensus | Kind::ReleaseClosure => {
            &[("resource_release_id", Bytes)]
        }
        Kind::MigrationRollbackRemoval => {
            &[("decision_closure_id", Bytes), ("effect_home_id", Bytes)]
        }
        Kind::StageProofMatrix => &[
            ("stage0_proof_manifest_id", Bytes),
            ("stage0_proof_manifest_artifact_sha256", Bytes),
            ("stage0_proof_gate_count", Unsigned),
        ],
        Kind::NormativeInputs => &[],
    }
}

fn field_type(kind: FacetFieldTypeV1) -> TypeExprV1 {
    match kind {
        FacetFieldTypeV1::Bytes => TypeExprV1::ExactBytes(32),
        FacetFieldTypeV1::BytesList => {
            TypeExprV1::OrderedList(Box::new(TypeExprV1::ExactBytes(32)))
        }
        FacetFieldTypeV1::Unsigned => TypeExprV1::Unsigned,
    }
}

fn record_schema(
    name: &str,
    fields: Vec<(&str, TypeExprV1)>,
) -> Result<SchemaDescriptorV1, SchemaError> {
    SchemaDescriptorV1::new(
        name,
        1,
        fields
            .into_iter()
            .enumerate()
            .map(|(index, (field_name, type_expr))| {
                FieldDescriptorV1::new(
                    u64::try_from(index + 1)
                        .expect("invariant: candidate root field count fits u64"),
                    field_name,
                    type_expr,
                    vec![ConstraintExprV1::NoAdditional],
                )
            })
            .collect::<Result<_, _>>()?,
        vec![],
    )
}

fn schema_id(schema_closure: &SchemaClosureV1, name: &str) -> Result<SchemaIdV1, SchemaError> {
    schema_closure
        .schema_id(name, 1)
        .copied()
        .ok_or(SchemaError::UnknownSchemaId)
}
