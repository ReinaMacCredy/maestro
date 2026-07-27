use maestro::domain::contract::component::{CandidateContractComponentV1, ContractComponentError};
use maestro::domain::contract::component_kind::{ComponentKindError, ContractComponentKindV1};
use maestro::domain::contract::finalization::{
    DesignBasisV1, DesignFinalizationError, DesignFinalizationManifestV1, FinalizationInputKindV1,
    PinnedFinalizationInputV1,
};
use maestro::domain::contract::handoff::{BuildHandoffError, CanonicalBuildHandoffV1};
use maestro::domain::contract::provenance::{ComponentProvenanceV1, ProvenanceError};
use maestro::domain::contract::root::{CandidateContractRootV1, ContractRootError};
use maestro::domain::identity::{
    ConstraintExprV1, ContractComponentIdV1, ContractRootIdV1, DescriptorDomainV1,
    DesignFinalizationManifestIdV1, EnumVariantV1, FieldDescriptorV1, FieldPathV1,
    ManifestDomainV1, ManifestHeaderV1, ManifestIdentityError, ManifestRowV1, ManifestValueV1,
    PathStepV1, SchemaClosureV1, SchemaDescriptorV1, SchemaError, SchemaIdV1, SchemaReferenceV1,
    TypeExprV1, decision_closure_identity, decision_resolution_identity,
    design_closure_requirement_identity, design_revision_identity, design_source_binding_identity,
    no_design_exemption_identity, optional_value_v1,
};
use maestro::foundation::core::deterministic_cbor::{CborError, CborValue, encode, validate};
use sha2::{Digest, Sha256};

fn no_additional() -> Vec<ConstraintExprV1> {
    vec![ConstraintExprV1::NoAdditional]
}

fn unsigned_schema(name: &str) -> SchemaDescriptorV1 {
    SchemaDescriptorV1::new(
        name,
        1,
        vec![
            FieldDescriptorV1::new(1, "value", TypeExprV1::Unsigned, no_additional())
                .expect("valid field"),
        ],
        vec![],
    )
    .expect("valid schema")
}

fn record_schema(name: &str, fields: Vec<(&str, TypeExprV1)>) -> SchemaDescriptorV1 {
    SchemaDescriptorV1::new(
        name,
        1,
        fields
            .into_iter()
            .enumerate()
            .map(|(index, (name, type_expr))| {
                FieldDescriptorV1::new(
                    u64::try_from(index + 1).expect("small field index"),
                    name,
                    type_expr,
                    no_additional(),
                )
                .expect("valid field")
            })
            .collect(),
        vec![],
    )
    .expect("valid record schema")
}

fn stage0_schema_closure() -> SchemaClosureV1 {
    let mut schemas = vec![
        record_schema(
            "Stage0ComponentValueV1",
            vec![
                ("component_kind", TypeExprV1::Unsigned),
                ("seed", TypeExprV1::Unsigned),
            ],
        ),
        record_schema(
            "ObservationDescriptorV1",
            vec![
                ("numeric_tag", TypeExprV1::Unsigned),
                ("name", TypeExprV1::AsciiText),
            ],
        ),
        record_schema(
            "AlternateDescriptorV1",
            vec![
                ("numeric_tag", TypeExprV1::Unsigned),
                ("name", TypeExprV1::AsciiText),
            ],
        ),
        record_schema(
            "ObservationManifestV1",
            vec![
                ("header_version", TypeExprV1::Unsigned),
                ("row_count", TypeExprV1::Unsigned),
                ("maximum_tag", TypeExprV1::Unsigned),
                ("catalog_version", TypeExprV1::Unsigned),
            ],
        ),
    ];
    schemas.extend(FinalizationInputKindV1::ALL.into_iter().map(|kind| {
        record_schema(
            kind.schema_name(),
            vec![
                ("input_kind", TypeExprV1::Unsigned),
                ("seed", TypeExprV1::Unsigned),
            ],
        )
    }));
    SchemaClosureV1::new(schemas).expect("stage-0 schema closure")
}

fn component_schema_id(schema_closure: &SchemaClosureV1) -> SchemaIdV1 {
    *schema_closure
        .schema_id("Stage0ComponentValueV1", 1)
        .expect("component schema id")
}

fn design_provenance(slot_tag: u64, seed: u64) -> ComponentProvenanceV1 {
    let design_revision_id = design_revision_identity(&CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(seed),
    ]))
    .expect("design revision id");
    let source_binding_id = design_source_binding_identity(&CborValue::Array(vec![
        CborValue::Unsigned(slot_tag),
        CborValue::Unsigned(seed),
    ]))
    .expect("source binding id");
    ComponentProvenanceV1::design_slot(design_revision_id, slot_tag, source_binding_id)
        .expect("design provenance")
}

fn component_for(
    schema_closure: &SchemaClosureV1,
    kind: ContractComponentKindV1,
    seed: u64,
    dependencies: Vec<ContractComponentIdV1>,
) -> CandidateContractComponentV1 {
    CandidateContractComponentV1::new(
        schema_closure,
        kind,
        component_schema_id(schema_closure),
        CborValue::Array(vec![
            CborValue::Unsigned(kind.tag()),
            CborValue::Unsigned(seed),
        ]),
        dependencies,
        design_provenance(kind.tag(), seed),
    )
    .expect("candidate component")
}

fn complete_components(
    schema_closure: &SchemaClosureV1,
    seed: u64,
) -> Vec<CandidateContractComponentV1> {
    ContractComponentKindV1::ALL
        .into_iter()
        .map(|kind| component_for(schema_closure, kind, seed, vec![]))
        .collect()
}

fn complete_finalization_inputs(
    schema_closure: &SchemaClosureV1,
    seed: u64,
) -> Vec<PinnedFinalizationInputV1> {
    FinalizationInputKindV1::ALL
        .into_iter()
        .map(|kind| {
            let value = CborValue::Array(vec![
                CborValue::Unsigned(kind.tag()),
                CborValue::Unsigned(seed),
            ]);
            match kind {
                FinalizationInputKindV1::ClosureRequirement => {
                    PinnedFinalizationInputV1::closure_requirement(schema_closure, value)
                }
                FinalizationInputKindV1::DeterministicSynthesis => {
                    PinnedFinalizationInputV1::deterministic_synthesis(schema_closure, value)
                }
                FinalizationInputKindV1::ScopeAndExclusions => {
                    PinnedFinalizationInputV1::scope_and_exclusions(schema_closure, value)
                }
                FinalizationInputKindV1::CapabilityCensusAndJourneys => {
                    PinnedFinalizationInputV1::capability_census_and_journeys(schema_closure, value)
                }
                FinalizationInputKindV1::MigrationRollbackRemoval => {
                    PinnedFinalizationInputV1::migration_rollback_removal(schema_closure, value)
                }
                FinalizationInputKindV1::StageProofMatrix => {
                    PinnedFinalizationInputV1::stage_proof_matrix(schema_closure, value)
                }
                FinalizationInputKindV1::ReviewEvidence => {
                    PinnedFinalizationInputV1::review_evidence(schema_closure, value)
                }
                FinalizationInputKindV1::EdgeSweepEvidence => {
                    PinnedFinalizationInputV1::edge_sweep_evidence(schema_closure, value)
                }
                FinalizationInputKindV1::RiskRecovery => {
                    PinnedFinalizationInputV1::risk_recovery(schema_closure, value)
                }
                FinalizationInputKindV1::FreshnessReferences => {
                    PinnedFinalizationInputV1::freshness_references(schema_closure, value)
                }
                FinalizationInputKindV1::CanonicalizationPolicy => {
                    PinnedFinalizationInputV1::canonicalization_policy(schema_closure, value)
                }
            }
            .expect("pinned input")
        })
        .collect()
}

fn finalization_for(
    schema_closure: &SchemaClosureV1,
    root: &CandidateContractRootV1,
    decision_seed: u64,
    input_seed: u64,
) -> DesignFinalizationManifestV1 {
    let design_revision_id = design_revision_identity(&CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(input_seed),
    ]))
    .expect("design revision");
    let decision_closure_id = decision_closure_identity(&CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(decision_seed),
    ]))
    .expect("decision closure");
    DesignFinalizationManifestV1::new(
        schema_closure,
        DesignBasisV1::design_revision(design_revision_id),
        decision_closure_id,
        root,
        complete_finalization_inputs(schema_closure, input_seed),
    )
    .expect("finalization")
}

fn reference_encode(value: &CborValue) -> Vec<u8> {
    fn append_head(output: &mut Vec<u8>, major: u8, value: u64) {
        let prefix = major << 5;
        match value {
            0..=23 => output.push(prefix | value as u8),
            24..=0xff => output.extend_from_slice(&[prefix | 24, value as u8]),
            0x100..=0xffff => {
                output.push(prefix | 25);
                output.extend_from_slice(&(value as u16).to_be_bytes());
            }
            0x1_0000..=0xffff_ffff => {
                output.push(prefix | 26);
                output.extend_from_slice(&(value as u32).to_be_bytes());
            }
            _ => {
                output.push(prefix | 27);
                output.extend_from_slice(&value.to_be_bytes());
            }
        }
    }

    fn encode_into(value: &CborValue, output: &mut Vec<u8>) {
        match value {
            CborValue::Unsigned(value) => append_head(output, 0, *value),
            CborValue::Bool(false) => output.push(0xf4),
            CborValue::Bool(true) => output.push(0xf5),
            CborValue::Bytes(value) => {
                append_head(output, 2, value.len() as u64);
                output.extend_from_slice(value);
            }
            CborValue::Text(value) => {
                assert!(value.is_ascii(), "reference encoder accepts ASCII only");
                append_head(output, 3, value.len() as u64);
                output.extend_from_slice(value.as_bytes());
            }
            CborValue::Array(values) => {
                append_head(output, 4, values.len() as u64);
                for value in values {
                    encode_into(value, output);
                }
            }
        }
    }

    let mut output = Vec::new();
    encode_into(value, &mut output);
    output
}

fn reference_hash_array(values: Vec<CborValue>) -> [u8; 32] {
    Sha256::digest(reference_encode(&CborValue::Array(values))).into()
}

fn reference_domain_identity(domain: &str, value: CborValue) -> [u8; 32] {
    reference_hash_array(vec![CborValue::Text(domain.to_owned()), value])
}

fn reference_schema_value_valid(
    closure: &SchemaClosureV1,
    schema_id: &SchemaIdV1,
    value: &CborValue,
) -> bool {
    let Some(descriptor) = closure.descriptor_for_id(schema_id) else {
        return false;
    };
    let CborValue::Array(values) = value else {
        return false;
    };
    if values.len() != descriptor.fields().len() || !descriptor.cross_constraints().is_empty() {
        return false;
    }
    descriptor
        .fields()
        .iter()
        .zip(values)
        .all(|(field, value)| {
            reference_type_matches(closure, field.type_expr(), value)
                && field
                    .constraints()
                    .iter()
                    .all(|constraint| reference_constraint_matches(constraint, value))
        })
}

fn reference_type_matches(
    closure: &SchemaClosureV1,
    type_expr: &TypeExprV1,
    value: &CborValue,
) -> bool {
    match (type_expr, value) {
        (TypeExprV1::Unsigned, CborValue::Unsigned(_))
        | (TypeExprV1::Boolean, CborValue::Bool(_)) => true,
        (TypeExprV1::AsciiText, CborValue::Text(value)) => value.is_ascii(),
        (TypeExprV1::ExactBytes(expected), CborValue::Bytes(value)) => {
            u64::try_from(value.len()).ok() == Some(*expected)
        }
        (TypeExprV1::SchemaReference(reference), value) => closure
            .schema_id(reference.schema_name(), reference.schema_version())
            .filter(|schema_id| schema_id.as_bytes() == reference.claimed_schema_id())
            .is_some_and(|schema_id| reference_schema_value_valid(closure, schema_id, value)),
        (TypeExprV1::Optional(inner), CborValue::Array(values)) => match values.as_slice() {
            [CborValue::Unsigned(0)] => true,
            [CborValue::Unsigned(1), value] => reference_type_matches(closure, inner, value),
            _ => false,
        },
        (TypeExprV1::OrderedList(inner), CborValue::Array(values)) => values
            .iter()
            .all(|value| reference_type_matches(closure, inner, value)),
        (TypeExprV1::Tuple(types), CborValue::Array(values)) => {
            types.len() == values.len()
                && types
                    .iter()
                    .zip(values)
                    .all(|(type_expr, value)| reference_type_matches(closure, type_expr, value))
        }
        (TypeExprV1::ClosedEnum { variants, .. }, CborValue::Unsigned(selected)) => {
            variants.iter().any(|variant| variant.tag() == *selected)
        }
        _ => false,
    }
}

fn reference_constraint_matches(constraint: &ConstraintExprV1, value: &CborValue) -> bool {
    match constraint {
        ConstraintExprV1::NoAdditional => true,
        ConstraintExprV1::BoundedLength { minimum, maximum } => {
            let length = match value {
                CborValue::Bytes(value) => value.len(),
                CborValue::Text(value) => value.len(),
                CborValue::Array(value) => value.len(),
                CborValue::Unsigned(_) | CborValue::Bool(_) => return false,
            } as u64;
            length >= *minimum && length <= *maximum
        }
        ConstraintExprV1::UnsignedRange { minimum, maximum } => {
            matches!(value, CborValue::Unsigned(value) if value >= minimum && value <= maximum)
        }
        ConstraintExprV1::CanonicalSet { .. } | ConstraintExprV1::ExactFieldEquality(_) => false,
    }
}

fn reference_provenance_value(provenance: &ComponentProvenanceV1) -> CborValue {
    match provenance {
        ComponentProvenanceV1::DesignSlot(value) => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(value.design_revision_id().as_bytes().to_vec()),
            CborValue::Unsigned(value.slot_tag()),
            CborValue::Bytes(value.source_binding_id().as_bytes().to_vec()),
        ]),
        ComponentProvenanceV1::AuthorizedNoDesign(value) => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(value.closure_requirement_id().as_bytes().to_vec()),
            CborValue::Bytes(value.exemption_id().as_bytes().to_vec()),
            CborValue::Bytes(value.source_binding_id().as_bytes().to_vec()),
        ]),
        ComponentProvenanceV1::DecisionMaterialization(value) => CborValue::Array(vec![
            CborValue::Unsigned(3),
            CborValue::Bytes(value.resolution_id().as_bytes().to_vec()),
            CborValue::Bytes(value.materialization_id().as_bytes().to_vec()),
        ]),
        ComponentProvenanceV1::DecisionMaterializationPreimage(value) => CborValue::Array(vec![
            CborValue::Unsigned(4),
            CborValue::Bytes(value.resolution_id().as_bytes().to_vec()),
            CborValue::Bytes(value.commitment().as_bytes().to_vec()),
        ]),
    }
}

fn reference_component_value(component: &CandidateContractComponentV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(component.kind().tag()),
        CborValue::Bytes(component.schema_id().as_bytes().to_vec()),
        component.value().clone(),
        CborValue::Array(
            component
                .dependencies()
                .iter()
                .map(|dependency| CborValue::Bytes(dependency.as_bytes().to_vec()))
                .collect(),
        ),
        reference_provenance_value(component.provenance()),
    ])
}

fn reference_root_value(root: &CandidateContractRootV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(root.components().len() as u64),
        CborValue::Array(
            root.components()
                .iter()
                .map(|component| {
                    CborValue::Array(vec![
                        CborValue::Bytes(component.component_id().as_bytes().to_vec()),
                        reference_component_value(component),
                    ])
                })
                .collect(),
        ),
    ])
}

fn reference_finalization_value(finalization: &DesignFinalizationManifestV1) -> CborValue {
    let basis = match finalization.design_basis() {
        DesignBasisV1::DesignRevision(design_revision_id) => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(design_revision_id.as_bytes().to_vec()),
        ]),
        DesignBasisV1::AuthorizedNoDesign {
            closure_requirement_id,
            exemption_id,
            source_binding_id,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(closure_requirement_id.as_bytes().to_vec()),
            CborValue::Bytes(exemption_id.as_bytes().to_vec()),
            CborValue::Bytes(source_binding_id.as_bytes().to_vec()),
        ]),
    };
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        basis,
        CborValue::Bytes(finalization.decision_closure_id().as_bytes().to_vec()),
        CborValue::Bytes(
            finalization
                .candidate_contract_root_id()
                .as_bytes()
                .to_vec(),
        ),
        CborValue::Array(
            finalization
                .pinned_inputs()
                .iter()
                .map(|input| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(input.kind().tag()),
                        CborValue::Bytes(input.schema_id().as_bytes().to_vec()),
                        CborValue::Bytes(input.input_id().as_bytes().to_vec()),
                    ])
                })
                .collect(),
        ),
    ])
}

fn reference_handoff_value(
    finalization: &DesignFinalizationManifestV1,
    root: &CandidateContractRootV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(finalization.manifest_id().as_bytes().to_vec()),
        CborValue::Bytes(root.root_id().as_bytes().to_vec()),
        CborValue::Array(
            root.components()
                .iter()
                .map(|component| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(component.kind().tag()),
                        CborValue::Bytes(component.component_id().as_bytes().to_vec()),
                    ])
                })
                .collect(),
        ),
        CborValue::Array(
            finalization
                .pinned_inputs()
                .iter()
                .map(|input| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(input.kind().tag()),
                        CborValue::Bytes(input.schema_id().as_bytes().to_vec()),
                        CborValue::Bytes(input.input_id().as_bytes().to_vec()),
                    ])
                })
                .collect(),
        ),
    ])
}

#[test]
fn deterministic_cbor_golden_vector_uses_shortest_forms() {
    let value = CborValue::Array(vec![
        CborValue::Unsigned(0),
        CborValue::Unsigned(23),
        CborValue::Unsigned(24),
        CborValue::Unsigned(255),
        CborValue::Unsigned(256),
        CborValue::Unsigned(65_535),
        CborValue::Unsigned(65_536),
        CborValue::Bool(false),
        CborValue::Bool(true),
        CborValue::Bytes(vec![0, 255]),
        CborValue::text("A").expect("ASCII"),
    ]);

    let expected = vec![
        0x8b, 0x00, 0x17, 0x18, 0x18, 0x18, 0xff, 0x19, 0x01, 0x00, 0x19, 0xff, 0xff, 0x1a, 0x00,
        0x01, 0x00, 0x00, 0xf4, 0xf5, 0x42, 0x00, 0xff, 0x61, 0x41,
    ];
    assert_eq!(encode(&value).expect("encodes"), expected);
    assert_eq!(validate(&expected), Ok(()));
}

#[test]
fn deterministic_cbor_rejects_every_forbidden_family() {
    let invalid = [
        (vec![0x20], CborError::UnsupportedMajorType(1)),
        (vec![0xa0], CborError::UnsupportedMajorType(5)),
        (vec![0xc0, 0x00], CborError::UnsupportedMajorType(6)),
        (vec![0xf6], CborError::UnsupportedSimpleValue(0xf6)),
        (
            vec![0xf9, 0x00, 0x00],
            CborError::UnsupportedSimpleValue(0xf9),
        ),
        (vec![0x9f, 0xff], CborError::IndefiniteLength),
        (vec![0x62, 0xc3, 0xa9], CborError::NonAsciiText),
        (vec![0x18, 0x17], CborError::NonCanonicalInteger),
    ];
    for (bytes, expected) in invalid {
        assert_eq!(validate(&bytes), Err(expected), "bytes={bytes:02x?}");
    }
    assert_eq!(
        encode(&CborValue::Text("cafe\u{301}".to_owned())),
        Err(CborError::NonAsciiText)
    );
}

#[test]
fn optional_values_have_one_exact_array_encoding() {
    assert_eq!(
        encode(&optional_value_v1(None)).expect("absent"),
        vec![0x81, 0x00]
    );
    assert_eq!(
        encode(&optional_value_v1(Some(CborValue::Array(vec![])))).expect("present empty"),
        vec![0x82, 0x01, 0x80]
    );
    assert_eq!(
        encode(&optional_value_v1(Some(CborValue::Unsigned(0)))).expect("present zero"),
        vec![0x82, 0x01, 0x00]
    );
}

#[test]
fn schema_descriptor_has_golden_bytes_and_lowercase_identity() {
    let descriptor = SchemaDescriptorV1::new(
        "Example",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "count",
                TypeExprV1::Unsigned,
                vec![ConstraintExprV1::UnsignedRange {
                    minimum: 0,
                    maximum: 9,
                }],
            )
            .expect("valid field"),
        ],
        vec![],
    )
    .expect("valid schema");
    let expected = vec![
        0x84, 0x67, b'E', b'x', b'a', b'm', b'p', b'l', b'e', 0x01, 0x81, 0x84, 0x01, 0x65, b'c',
        b'o', b'u', b'n', b't', 0x81, 0x01, 0x81, 0x83, 0x04, 0x00, 0x09, 0x80,
    ];
    assert_eq!(descriptor.canonical_bytes().expect("bytes"), expected);
    let rendered = descriptor.schema_id().expect("identity").render();
    assert_eq!(
        rendered,
        "sha256:9223e34869125b1018de6944c6f5c79c70ce2a8e0a94c818bafc716bfee68881"
    );
    assert!(rendered.starts_with("sha256:"));
    assert_eq!(rendered.len(), 71);
    assert!(
        rendered[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

#[test]
fn schema_validation_rejects_non_ascii_duplicates_and_unsorted_sets() {
    let non_ascii = FieldDescriptorV1::new(1, "caf\u{e9}", TypeExprV1::Unsigned, no_additional());
    assert!(matches!(non_ascii, Err(SchemaError::InvalidAsciiName(_))));

    let duplicate_name = SchemaDescriptorV1::new(
        "Duplicate",
        1,
        vec![
            FieldDescriptorV1::new(1, "same", TypeExprV1::Unsigned, no_additional())
                .expect("field"),
            FieldDescriptorV1::new(2, "same", TypeExprV1::Unsigned, no_additional())
                .expect("field"),
        ],
        vec![],
    );
    assert!(matches!(
        duplicate_name,
        Err(SchemaError::DuplicateFieldName(_))
    ));

    let unsorted_positions = SchemaDescriptorV1::new(
        "Unsorted",
        1,
        vec![
            FieldDescriptorV1::new(2, "two", TypeExprV1::Unsigned, no_additional()).expect("field"),
            FieldDescriptorV1::new(1, "one", TypeExprV1::Unsigned, no_additional()).expect("field"),
        ],
        vec![],
    );
    assert!(matches!(
        unsorted_positions,
        Err(SchemaError::FieldsNotStrictlyPositionSorted)
    ));

    let unsorted_constraints = FieldDescriptorV1::new(
        1,
        "value",
        TypeExprV1::Unsigned,
        vec![
            ConstraintExprV1::UnsignedRange {
                minimum: 10,
                maximum: 20,
            },
            ConstraintExprV1::UnsignedRange {
                minimum: 0,
                maximum: 9,
            },
        ],
    );
    assert!(matches!(
        unsorted_constraints,
        Err(SchemaError::ConstraintsNotStrictlySorted)
    ));

    let unsorted_enum_tags = FieldDescriptorV1::new(
        1,
        "state",
        TypeExprV1::ClosedEnum {
            enum_name: "State".to_owned(),
            variants: vec![
                EnumVariantV1::new(2, "two").expect("variant"),
                EnumVariantV1::new(1, "one").expect("variant"),
            ],
        },
        no_additional(),
    );
    assert!(matches!(
        unsorted_enum_tags,
        Err(SchemaError::EnumTagsNotStrictlySorted)
    ));

    let duplicate_enum_name = FieldDescriptorV1::new(
        1,
        "state",
        TypeExprV1::ClosedEnum {
            enum_name: "State".to_owned(),
            variants: vec![
                EnumVariantV1::new(1, "same").expect("variant"),
                EnumVariantV1::new(2, "same").expect("variant"),
            ],
        },
        no_additional(),
    );
    assert!(matches!(
        duplicate_enum_name,
        Err(SchemaError::DuplicateEnumName(name)) if name == "same"
    ));
}

#[test]
fn schema_identity_requires_semantically_valid_paths_and_reference_closure() {
    let invalid_set_key = SchemaDescriptorV1::new(
        "InvalidSetKey",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "rows",
                TypeExprV1::OrderedList(Box::new(TypeExprV1::Tuple(vec![TypeExprV1::AsciiText]))),
                vec![ConstraintExprV1::CanonicalSet {
                    key_path: FieldPathV1::new(vec![PathStepV1::TupleIndex(0)]).expect("path"),
                    minimum: 0,
                    maximum: 10,
                }],
            )
            .expect("field"),
        ],
        vec![],
    )
    .expect("locally valid schema");
    assert!(matches!(
        invalid_set_key.schema_id(),
        Err(SchemaError::CanonicalSetKeyMustBeUnsigned)
    ));

    let base = unsigned_schema("IdentityBase");
    let base_id = base.schema_id().expect("base identity");
    let referencing = SchemaDescriptorV1::new(
        "IdentityDependent",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "base",
                TypeExprV1::SchemaReference(
                    SchemaReferenceV1::verified("IdentityBase", 1, &base_id).expect("reference"),
                ),
                no_additional(),
            )
            .expect("field"),
        ],
        vec![],
    )
    .expect("dependent schema");
    assert!(matches!(
        referencing.schema_id(),
        Err(SchemaError::SchemaReferencesRequireClosure)
    ));
    assert!(SchemaClosureV1::new(vec![base, referencing]).is_ok());
}

#[test]
fn schema_closure_rejects_unknown_ids_wrong_shapes_types_and_constraint_values() {
    let schema_closure = stage0_schema_closure();
    let component_schema_id = component_schema_id(&schema_closure);
    let valid = CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(2)]);
    assert!(
        schema_closure
            .validate_value(&component_schema_id, &valid)
            .is_ok()
    );
    assert!(matches!(
        schema_closure.validate_value(
            &component_schema_id,
            &CborValue::Array(vec![CborValue::Unsigned(1)])
        ),
        Err(SchemaError::SchemaValueShapeMismatch)
    ));
    assert!(matches!(
        schema_closure.validate_value(
            &component_schema_id,
            &CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(2),
                CborValue::Unsigned(3),
            ])
        ),
        Err(SchemaError::SchemaValueShapeMismatch)
    ));
    assert!(matches!(
        schema_closure.validate_value(
            &component_schema_id,
            &CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Bool(false)])
        ),
        Err(SchemaError::ValueTypeMismatch)
    ));
    let external_schema_id = unsigned_schema("ExternalOnly").schema_id().unwrap();
    assert!(matches!(
        schema_closure.validate_value(&external_schema_id, &valid),
        Err(SchemaError::UnknownSchemaId)
    ));

    let set_schema = SchemaDescriptorV1::new(
        "CanonicalSetValueV1",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "rows",
                TypeExprV1::OrderedList(Box::new(TypeExprV1::Tuple(vec![
                    TypeExprV1::Unsigned,
                    TypeExprV1::AsciiText,
                ]))),
                vec![ConstraintExprV1::CanonicalSet {
                    key_path: FieldPathV1::new(vec![PathStepV1::TupleIndex(0)]).unwrap(),
                    minimum: 1,
                    maximum: 3,
                }],
            )
            .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    let set_closure = SchemaClosureV1::new(vec![set_schema]).unwrap();
    let set_schema_id = *set_closure.schema_id("CanonicalSetValueV1", 1).unwrap();
    let row = |tag, label| {
        CborValue::Array(vec![
            CborValue::Unsigned(tag),
            CborValue::text(label).unwrap(),
        ])
    };
    let sorted = CborValue::Array(vec![CborValue::Array(vec![row(1, "one"), row(2, "two")])]);
    assert!(set_closure.validate_value(&set_schema_id, &sorted).is_ok());
    let unsorted = CborValue::Array(vec![CborValue::Array(vec![row(2, "two"), row(1, "one")])]);
    assert!(matches!(
        set_closure.validate_value(&set_schema_id, &unsorted),
        Err(SchemaError::CanonicalSetValuesNotStrictlySorted)
    ));
}

#[test]
fn schema_closure_accepts_only_content_bound_backward_references() {
    let base = unsigned_schema("Base");
    let base_id = base.schema_id().expect("base id");
    let dependent = SchemaDescriptorV1::new(
        "Dependent",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "base",
                TypeExprV1::SchemaReference(
                    SchemaReferenceV1::verified("Base", 1, &base_id).expect("reference"),
                ),
                no_additional(),
            )
            .expect("field"),
        ],
        vec![],
    )
    .expect("dependent");
    assert!(SchemaClosureV1::new(vec![base.clone(), dependent.clone()]).is_ok());
    assert!(matches!(
        SchemaClosureV1::new(vec![dependent, base]),
        Err(SchemaError::ForwardOrCyclicSchemaReference { .. })
    ));

    let unknown = SchemaDescriptorV1::new(
        "UnknownUser",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "missing",
                TypeExprV1::SchemaReference(
                    SchemaReferenceV1::claimed("Missing", 1, [0; 32]).expect("claim"),
                ),
                no_additional(),
            )
            .expect("field"),
        ],
        vec![],
    )
    .expect("schema");
    assert!(matches!(
        SchemaClosureV1::new(vec![unknown]),
        Err(SchemaError::UnknownSchemaReference { .. })
    ));

    let cyclic = SchemaDescriptorV1::new(
        "Cycle",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "self_ref",
                TypeExprV1::SchemaReference(
                    SchemaReferenceV1::claimed("Cycle", 1, [0; 32]).expect("claim"),
                ),
                no_additional(),
            )
            .expect("field"),
        ],
        vec![],
    )
    .expect("schema");
    assert!(matches!(
        SchemaClosureV1::new(vec![cyclic]),
        Err(SchemaError::ForwardOrCyclicSchemaReference { .. })
    ));
}

#[test]
fn manifest_helpers_bind_fixed_paired_domains_and_sorted_rows() {
    let schema_closure = stage0_schema_closure();
    let descriptor_schema_id = *schema_closure
        .schema_id("ObservationDescriptorV1", 1)
        .unwrap();
    let manifest_schema_id = *schema_closure
        .schema_id("ObservationManifestV1", 1)
        .unwrap();
    let row_one = ManifestRowV1::new(
        &schema_closure,
        1,
        DescriptorDomainV1::ObservationKind,
        &descriptor_schema_id,
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text("one").unwrap(),
        ]),
    )
    .unwrap();
    let row_two = ManifestRowV1::new(
        &schema_closure,
        2,
        DescriptorDomainV1::ObservationKind,
        &descriptor_schema_id,
        CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::text("two").unwrap(),
        ]),
    )
    .unwrap();
    let manifest = ManifestValueV1::new(
        &schema_closure,
        ManifestDomainV1::ObservationKind,
        manifest_schema_id,
        descriptor_schema_id,
        ManifestHeaderV1::new(2, 2, CborValue::Unsigned(1)).unwrap(),
        vec![row_one.clone(), row_two.clone()],
    )
    .unwrap();
    assert_eq!(
        row_one.descriptor_id().render(),
        "sha256:a044df727da4a45fe6329ad06bf0a4aa71308cba76037d4d504389a0e24ece36"
    );
    assert_eq!(
        manifest.manifest_id().render(),
        "sha256:eb0a14984d05b580adce707b6b6750a8cd1c97785cc15fdddc3de5a9d0a89c0e"
    );

    let wider_header = ManifestValueV1::new(
        &schema_closure,
        ManifestDomainV1::ObservationKind,
        manifest_schema_id,
        descriptor_schema_id,
        ManifestHeaderV1::new(2, 3, CborValue::Unsigned(1)).unwrap(),
        vec![row_one.clone(), row_two.clone()],
    )
    .unwrap();
    assert_ne!(manifest.manifest_id(), wider_header.manifest_id());
    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::ObservationKind,
            manifest_schema_id,
            descriptor_schema_id,
            ManifestHeaderV1::new(2, 1, CborValue::Unsigned(1)).unwrap(),
            vec![row_one.clone(), row_two.clone()],
        ),
        Err(ManifestIdentityError::RowTagOutsideHeaderBound)
    ));
    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::ObservationKind,
            manifest_schema_id,
            descriptor_schema_id,
            ManifestHeaderV1::new(1, 2, CborValue::Unsigned(1)).unwrap(),
            vec![row_one.clone(), row_two.clone()],
        ),
        Err(ManifestIdentityError::HeaderRowCountMismatch)
    ));
    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::ObservationKind,
            component_schema_id(&schema_closure),
            descriptor_schema_id,
            ManifestHeaderV1::new(2, 2, CborValue::Unsigned(1)).unwrap(),
            vec![row_one.clone(), row_two.clone()],
        ),
        Err(ManifestIdentityError::Schema(
            SchemaError::SchemaValueShapeMismatch
        ))
    ));

    let alternate_schema_id = *schema_closure
        .schema_id("AlternateDescriptorV1", 1)
        .unwrap();
    let alternate_row = ManifestRowV1::new(
        &schema_closure,
        1,
        DescriptorDomainV1::ObservationKind,
        &alternate_schema_id,
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text("one").unwrap(),
        ]),
    )
    .unwrap();
    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::ObservationKind,
            manifest_schema_id,
            descriptor_schema_id,
            ManifestHeaderV1::new(1, 1, CborValue::Unsigned(1)).unwrap(),
            vec![alternate_row],
        ),
        Err(ManifestIdentityError::DescriptorSchemaMismatch)
    ));
    assert!(matches!(
        ManifestRowV1::new(
            &schema_closure,
            1,
            DescriptorDomainV1::ObservationKind,
            &descriptor_schema_id,
            CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::text("one").unwrap(),
                CborValue::Unsigned(3),
            ]),
        ),
        Err(ManifestIdentityError::Schema(
            SchemaError::SchemaValueShapeMismatch
        ))
    ));
    assert!(matches!(
        ManifestRowV1::new(
            &schema_closure,
            1,
            DescriptorDomainV1::ObservationKind,
            &descriptor_schema_id,
            CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(2)]),
        ),
        Err(ManifestIdentityError::Schema(
            SchemaError::ValueTypeMismatch
        ))
    ));

    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::ObservationKind,
            manifest_schema_id,
            descriptor_schema_id,
            ManifestHeaderV1::new(2, 2, CborValue::Unsigned(1)).unwrap(),
            vec![row_two, row_one.clone()],
        ),
        Err(ManifestIdentityError::RowsNotStrictlyTagSorted)
    ));
    assert!(matches!(
        ManifestValueV1::new(
            &schema_closure,
            ManifestDomainV1::EffectOrigin,
            manifest_schema_id,
            descriptor_schema_id,
            ManifestHeaderV1::new(1, 2, CborValue::Unsigned(1)).unwrap(),
            vec![row_one.clone()],
        ),
        Err(ManifestIdentityError::DescriptorIdentityMismatch)
    ));
}

#[test]
fn candidate_root_is_complete_and_independent_of_input_enumeration() {
    let schema_closure = stage0_schema_closure();
    let mut components = complete_components(&schema_closure, 7);
    components.push(component_for(
        &schema_closure,
        ContractComponentKindV1::NormativeInputs,
        8,
        vec![],
    ));
    let forward =
        CandidateContractRootV1::new(&schema_closure, components.clone()).expect("forward root");
    components.reverse();
    let reversed =
        CandidateContractRootV1::new(&schema_closure, components).expect("reversed root");
    assert_eq!(forward.root_id(), reversed.root_id());
    assert_eq!(
        forward.canonical_bytes().unwrap(),
        reversed.canonical_bytes().unwrap()
    );
    assert_eq!(
        forward.components().len(),
        ContractComponentKindV1::ALL.len() + 1
    );

    let ordered_kinds: Vec<_> = forward
        .components()
        .iter()
        .map(CandidateContractComponentV1::kind)
        .collect();
    assert_eq!(
        ordered_kinds
            .iter()
            .filter(|kind| **kind == ContractComponentKindV1::NormativeInputs)
            .count(),
        2
    );
}

#[test]
fn candidate_root_rejects_missing_kinds_duplicate_identities_and_unknown_dependencies() {
    let schema_closure = stage0_schema_closure();
    let mut missing = complete_components(&schema_closure, 1);
    missing.retain(|component| component.kind() != ContractComponentKindV1::IntendedOutcome);
    assert!(matches!(
        CandidateContractRootV1::new(&schema_closure, missing),
        Err(ContractRootError::MissingComponentKind(
            ContractComponentKindV1::IntendedOutcome
        ))
    ));

    let mut duplicate = complete_components(&schema_closure, 1);
    let duplicate_normative = duplicate
        .iter()
        .find(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
        .expect("normative component")
        .clone();
    duplicate.push(duplicate_normative);
    assert!(matches!(
        CandidateContractRootV1::new(&schema_closure, duplicate),
        Err(ContractRootError::DuplicateComponentIdentity)
    ));

    let old_outcome = component_for(
        &schema_closure,
        ContractComponentKindV1::IntendedOutcome,
        1,
        vec![],
    );
    let dependent = component_for(
        &schema_closure,
        ContractComponentKindV1::NormativeInputs,
        2,
        vec![*old_outcome.component_id()],
    );
    let mut unknown_dependency = complete_components(&schema_closure, 3);
    let normative_index = unknown_dependency
        .iter()
        .position(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
        .expect("normative component");
    unknown_dependency[normative_index] = dependent;
    assert!(matches!(
        CandidateContractRootV1::new(&schema_closure, unknown_dependency),
        Err(ContractRootError::UnknownComponentDependency)
    ));

    let mut dependency_ids = complete_components(&schema_closure, 4)
        .into_iter()
        .take(2)
        .map(|component| *component.component_id())
        .collect::<Vec<_>>();
    dependency_ids.sort();
    dependency_ids.reverse();
    assert!(matches!(
        CandidateContractComponentV1::new(
            &schema_closure,
            ContractComponentKindV1::NormativeInputs,
            component_schema_id(&schema_closure),
            CborValue::Array(vec![CborValue::Unsigned(12), CborValue::Unsigned(4)]),
            dependency_ids,
            design_provenance(12, 4),
        ),
        Err(ContractComponentError::DependenciesNotStrictlySorted)
    ));
}

#[test]
fn component_and_root_construction_reject_schema_label_substitution() {
    let schema_closure = stage0_schema_closure();
    let external_schema_id = unsigned_schema("ExternalComponentValueV1")
        .schema_id()
        .unwrap();
    let value = CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(1)]);
    assert!(matches!(
        CandidateContractComponentV1::new(
            &schema_closure,
            ContractComponentKindV1::IntendedOutcome,
            external_schema_id,
            value,
            vec![],
            design_provenance(1, 1),
        ),
        Err(ContractComponentError::Schema(SchemaError::UnknownSchemaId))
    ));

    let components = complete_components(&schema_closure, 5);
    let unrelated_closure = SchemaClosureV1::new(vec![record_schema(
        "UnrelatedComponentValueV1",
        vec![
            ("kind", TypeExprV1::Unsigned),
            ("seed", TypeExprV1::Unsigned),
        ],
    )])
    .unwrap();
    assert!(matches!(
        CandidateContractRootV1::new(&unrelated_closure, components),
        Err(ContractRootError::Schema(SchemaError::UnknownSchemaId))
    ));
}

#[test]
fn externally_constructible_component_provenance_is_variant_typed() {
    let schema_closure = stage0_schema_closure();
    let design_revision = design_revision_identity(&CborValue::Unsigned(1)).unwrap();
    let source_binding = design_source_binding_identity(&CborValue::Unsigned(2)).unwrap();
    let design = ComponentProvenanceV1::design_slot(design_revision, 1, source_binding).unwrap();

    let closure_requirement = design_closure_requirement_identity(&CborValue::Unsigned(3)).unwrap();
    let exemption = no_design_exemption_identity(&CborValue::Unsigned(4)).unwrap();
    let no_design =
        ComponentProvenanceV1::authorized_no_design(closure_requirement, exemption, source_binding);

    assert_eq!(design.variant_tag(), 1);
    assert_eq!(no_design.variant_tag(), 2);

    let schema_id = component_schema_id(&schema_closure);
    let value = CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(1)]);
    let ids = [design, no_design].map(|provenance| {
        *CandidateContractComponentV1::new(
            &schema_closure,
            ContractComponentKindV1::IntendedOutcome,
            schema_id,
            value.clone(),
            vec![],
            provenance,
        )
        .unwrap()
        .component_id()
    });
    assert_ne!(ids[0], ids[1]);
    assert_eq!(
        ComponentProvenanceV1::design_slot(design_revision, 0, source_binding),
        Err(ProvenanceError::InvalidDesignSlotTag)
    );
    assert_eq!(
        ComponentProvenanceV1::design_slot(design_revision, 65_536, source_binding),
        Err(ProvenanceError::InvalidDesignSlotTag)
    );
}

#[test]
fn complete_finalization_and_handoff_rotate_with_every_changed_input() {
    let schema_closure = stage0_schema_closure();
    let root =
        CandidateContractRootV1::new(&schema_closure, complete_components(&schema_closure, 11))
            .expect("root");
    let finalization = finalization_for(&schema_closure, &root, 21, 31);
    let handoff = CanonicalBuildHandoffV1::project(&finalization, &root).expect("handoff");
    assert_eq!(
        root.root_id().render(),
        "sha256:80156b969021579ea8a128489b5b180e6306d5688c77d99c439f2fe6d21fc13c"
    );
    assert_eq!(
        finalization.manifest_id().render(),
        "sha256:abf2db3f448a740b2f6672bef254fda29f61a0cf8cd3c645e050b8ca7ddd6dd0"
    );
    assert_eq!(
        handoff.handoff_id().render(),
        "sha256:cf36789af4627ac4022248ad6ba061fb621748196980c2851668ecbd5432b060"
    );
    assert_eq!(root.canonical_bytes().unwrap().len(), 3_537);
    assert_eq!(finalization.canonical_bytes().unwrap().len(), 877);
    assert_eq!(handoff.canonical_bytes().unwrap().len(), 1_708);
    let repeated_handoff =
        CanonicalBuildHandoffV1::project(&finalization, &root).expect("repeated handoff");
    assert_eq!(handoff.handoff_id(), repeated_handoff.handoff_id());
    assert_eq!(
        handoff.canonical_bytes().unwrap(),
        repeated_handoff.canonical_bytes().unwrap()
    );

    let changed_components_root =
        CandidateContractRootV1::new(&schema_closure, complete_components(&schema_closure, 12))
            .expect("changed root");
    assert!(matches!(
        CanonicalBuildHandoffV1::project(&finalization, &changed_components_root),
        Err(BuildHandoffError::CandidateRootMismatch)
    ));
    let changed_root_finalization =
        finalization_for(&schema_closure, &changed_components_root, 21, 31);
    let changed_root_handoff =
        CanonicalBuildHandoffV1::project(&changed_root_finalization, &changed_components_root)
            .expect("changed handoff");
    assert_ne!(root.root_id(), changed_components_root.root_id());
    assert_ne!(
        finalization.manifest_id(),
        changed_root_finalization.manifest_id()
    );
    assert_ne!(handoff.handoff_id(), changed_root_handoff.handoff_id());

    let changed_closure = finalization_for(&schema_closure, &root, 22, 31);
    assert_eq!(root.root_id(), changed_closure.candidate_contract_root_id());
    assert_ne!(finalization.manifest_id(), changed_closure.manifest_id());
    let changed_closure_handoff =
        CanonicalBuildHandoffV1::project(&changed_closure, &root).expect("handoff");
    assert_ne!(handoff.handoff_id(), changed_closure_handoff.handoff_id());

    let changed_pinned_input = finalization_for(&schema_closure, &root, 21, 32);
    assert_ne!(
        finalization.manifest_id(),
        changed_pinned_input.manifest_id()
    );
}

#[test]
fn independent_encoder_and_semantic_validator_reproduce_the_full_stage0_chain() {
    let schema_closure = stage0_schema_closure();
    let component_schema_id = component_schema_id(&schema_closure);
    let component_schema = schema_closure
        .descriptor_for_id(&component_schema_id)
        .expect("component schema");
    let component_schema_value = component_schema.canonical_value().unwrap();
    assert_eq!(
        reference_encode(&component_schema_value),
        component_schema.canonical_bytes().unwrap()
    );
    assert_eq!(
        reference_domain_identity("maestro.vnext.schema.v1", component_schema_value),
        *component_schema_id.as_bytes()
    );

    let root =
        CandidateContractRootV1::new(&schema_closure, complete_components(&schema_closure, 91))
            .unwrap();
    for component in root.components() {
        assert!(reference_schema_value_valid(
            &schema_closure,
            component.schema_id(),
            component.value()
        ));
        assert_eq!(
            reference_domain_identity(
                "maestro.vnext.contract-component.v1",
                reference_component_value(component),
            ),
            *component.component_id().as_bytes()
        );
    }
    let root_value = reference_root_value(&root);
    assert_eq!(
        reference_encode(&root_value),
        root.canonical_bytes().unwrap()
    );
    assert_eq!(
        reference_domain_identity("maestro.vnext.candidate-contract-root.v1", root_value),
        *root.root_id().as_bytes()
    );

    let finalization = finalization_for(&schema_closure, &root, 92, 93);
    for input in finalization.pinned_inputs() {
        assert!(reference_schema_value_valid(
            &schema_closure,
            input.schema_id(),
            input.value()
        ));
        let identity_value = CborValue::Array(vec![
            CborValue::Unsigned(input.kind().tag()),
            CborValue::Bytes(input.schema_id().as_bytes().to_vec()),
            input.value().clone(),
        ]);
        assert_eq!(
            reference_domain_identity("maestro.vnext.design-finalization-input.v1", identity_value),
            *input.input_id().as_bytes()
        );
    }
    let finalization_value = reference_finalization_value(&finalization);
    assert_eq!(
        reference_encode(&finalization_value),
        finalization.canonical_bytes().unwrap()
    );
    assert_eq!(
        reference_domain_identity(
            "maestro.vnext.design-finalization-manifest.v1",
            finalization_value,
        ),
        *finalization.manifest_id().as_bytes()
    );

    let handoff = CanonicalBuildHandoffV1::project(&finalization, &root).unwrap();
    let handoff_value = reference_handoff_value(&finalization, &root);
    assert_eq!(
        reference_encode(&handoff_value),
        handoff.canonical_bytes().unwrap()
    );
    assert_eq!(
        reference_domain_identity("maestro.vnext.build-handoff-projection.v1", handoff_value),
        *handoff.handoff_id().as_bytes()
    );

    let descriptor_schema_id = *schema_closure
        .schema_id("ObservationDescriptorV1", 1)
        .unwrap();
    let manifest_schema_id = *schema_closure
        .schema_id("ObservationManifestV1", 1)
        .unwrap();
    let row = ManifestRowV1::new(
        &schema_closure,
        1,
        DescriptorDomainV1::ObservationKind,
        &descriptor_schema_id,
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text("one").unwrap(),
        ]),
    )
    .unwrap();
    assert!(reference_schema_value_valid(
        &schema_closure,
        &descriptor_schema_id,
        row.descriptor_value()
    ));
    assert_eq!(
        reference_hash_array(vec![
            CborValue::text(DescriptorDomainV1::ObservationKind.as_str()).unwrap(),
            CborValue::Bytes(descriptor_schema_id.as_bytes().to_vec()),
            row.descriptor_value().clone(),
        ]),
        *row.descriptor_id().as_bytes()
    );
    let header = ManifestHeaderV1::new(1, 1, CborValue::Unsigned(1)).unwrap();
    assert!(reference_schema_value_valid(
        &schema_closure,
        &manifest_schema_id,
        &header.canonical_value()
    ));
    let manifest = ManifestValueV1::new(
        &schema_closure,
        ManifestDomainV1::ObservationKind,
        manifest_schema_id,
        descriptor_schema_id,
        header.clone(),
        vec![row.clone()],
    )
    .unwrap();
    let row_value = CborValue::Array(vec![
        CborValue::Unsigned(row.numeric_tag()),
        CborValue::Bytes(row.descriptor_id().as_bytes().to_vec()),
        row.descriptor_value().clone(),
    ]);
    assert_eq!(
        reference_hash_array(vec![
            CborValue::text(ManifestDomainV1::ObservationKind.as_str()).unwrap(),
            CborValue::Bytes(manifest_schema_id.as_bytes().to_vec()),
            CborValue::Bytes(descriptor_schema_id.as_bytes().to_vec()),
            header.canonical_value(),
            CborValue::Array(vec![row_value]),
        ]),
        *manifest.manifest_id().as_bytes()
    );

    assert!(!reference_schema_value_valid(
        &schema_closure,
        &component_schema_id,
        &CborValue::Array(vec![CborValue::Unsigned(1)])
    ));
    assert!(!reference_schema_value_valid(
        &schema_closure,
        &component_schema_id,
        &CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Bool(true)])
    ));
}

#[test]
fn rationale_only_decision_closure_stays_outside_root_but_inside_finalization() {
    let schema_closure = stage0_schema_closure();
    let root =
        CandidateContractRootV1::new(&schema_closure, complete_components(&schema_closure, 41))
            .expect("root");
    let before = finalization_for(&schema_closure, &root, 51, 61);
    let after_rationale_only_change = finalization_for(&schema_closure, &root, 52, 61);
    assert_eq!(
        before.candidate_contract_root_id(),
        after_rationale_only_change.candidate_contract_root_id()
    );
    assert_ne!(
        before.decision_closure_id(),
        after_rationale_only_change.decision_closure_id()
    );
    assert_ne!(
        before.manifest_id(),
        after_rationale_only_change.manifest_id()
    );
}

#[test]
fn finalization_rejects_incomplete_duplicate_and_unknown_input_kinds() {
    let schema_closure = stage0_schema_closure();
    let root =
        CandidateContractRootV1::new(&schema_closure, complete_components(&schema_closure, 71))
            .expect("root");
    let design_revision = design_revision_identity(&CborValue::Unsigned(72)).unwrap();
    let decision_closure = decision_closure_identity(&CborValue::Unsigned(73)).unwrap();

    let mut missing = complete_finalization_inputs(&schema_closure, 74);
    missing.retain(|input| input.kind() != FinalizationInputKindV1::ReviewEvidence);
    assert!(matches!(
        DesignFinalizationManifestV1::new(
            &schema_closure,
            DesignBasisV1::design_revision(design_revision),
            decision_closure,
            &root,
            missing,
        ),
        Err(DesignFinalizationError::MissingInputKind(
            FinalizationInputKindV1::ReviewEvidence
        ))
    ));

    let mut duplicate = complete_finalization_inputs(&schema_closure, 74);
    duplicate.push(
        PinnedFinalizationInputV1::review_evidence(
            &schema_closure,
            CborValue::Array(vec![CborValue::Unsigned(7), CborValue::Unsigned(75)]),
        )
        .unwrap(),
    );
    assert!(matches!(
        DesignFinalizationManifestV1::new(
            &schema_closure,
            DesignBasisV1::design_revision(design_revision),
            decision_closure,
            &root,
            duplicate,
        ),
        Err(DesignFinalizationError::DuplicateInputKind(
            FinalizationInputKindV1::ReviewEvidence
        ))
    ));

    assert!(matches!(
        FinalizationInputKindV1::try_from(0),
        Err(DesignFinalizationError::UnknownInputKind(0))
    ));
    assert_eq!(
        ContractComponentKindV1::try_from(25),
        Err(ComponentKindError::UnknownTag(25))
    );
    assert!(matches!(
        PinnedFinalizationInputV1::review_evidence(
            &schema_closure,
            CborValue::Array(vec![CborValue::Unsigned(7)])
        ),
        Err(DesignFinalizationError::Schema(
            SchemaError::SchemaValueShapeMismatch
        ))
    ));

    let incomplete_schema_closure = SchemaClosureV1::new(vec![record_schema(
        FinalizationInputKindV1::ReviewEvidence.schema_name(),
        vec![
            ("input_kind", TypeExprV1::Unsigned),
            ("seed", TypeExprV1::Unsigned),
        ],
    )])
    .unwrap();
    assert!(matches!(
        PinnedFinalizationInputV1::closure_requirement(
            &incomplete_schema_closure,
            CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(1)])
        ),
        Err(DesignFinalizationError::MissingInputSchema(
            FinalizationInputKindV1::ClosureRequirement
        ))
    ));
}

#[test]
fn fixed_domains_separate_identical_payloads_and_external_text_is_not_an_identity() {
    use std::any::TypeId;

    let payload = CborValue::Array(vec![CborValue::Unsigned(1)]);
    let schema = unsigned_schema("DomainSeparation").schema_id().unwrap();
    let design = design_revision_identity(&payload).unwrap();
    let decision = decision_resolution_identity(&payload).unwrap();
    assert_ne!(schema.as_bytes(), design.as_bytes());
    assert_ne!(design.as_bytes(), decision.as_bytes());

    assert_ne!(TypeId::of::<ContractRootIdV1>(), TypeId::of::<String>());
    assert_ne!(
        TypeId::of::<DesignFinalizationManifestIdV1>(),
        TypeId::of::<String>()
    );
}
