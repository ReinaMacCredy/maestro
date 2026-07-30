use maestro::domain::contract::assembly::{
    candidate_root_schema_closure_v1, facet_schema_id_v1, fixture_facet_value_v1,
    normative_inputs_schema_id_v1,
};
use maestro::domain::contract::component::CandidateContractComponentV1;
use maestro::domain::contract::component_kind::ContractComponentKindV1;
use maestro::domain::contract::materialization::{
    ContractConsequencePlanV1, PlannedContractComponentV1, PlannedContractDependencyV1,
    PlannedContractSlotV1,
};
use maestro::domain::contract::provenance::ComponentProvenanceV1;
use maestro::domain::contract::root::CandidateContractRootV1;
use maestro::domain::identity::{
    ContractComponentIdV1, DesignRevisionIdV1, DesignSourceBindingIdV1,
};
use maestro::foundation::core::deterministic_cbor::CborValue;

fn rendered(value: u8) -> String {
    format!("sha256:{}", format!("{value:02x}").repeat(32))
}

fn base_root() -> CandidateContractRootV1 {
    let schemas = candidate_root_schema_closure_v1().expect("schema closure");
    let design_revision_id = DesignRevisionIdV1::parse(&rendered(1)).expect("Design Revision");
    let source_binding_id = DesignSourceBindingIdV1::parse(&rendered(2)).expect("source binding");
    let components = ContractComponentKindV1::ALL
        .into_iter()
        .map(|kind| {
            let (schema_id, value) = if kind == ContractComponentKindV1::NormativeInputs {
                (
                    normative_inputs_schema_id_v1(&schemas).expect("normative schema"),
                    CborValue::Array(vec![
                        CborValue::Unsigned(1),
                        CborValue::Bytes([3; 32].to_vec()),
                        CborValue::Array(Vec::new()),
                    ]),
                )
            } else {
                (
                    facet_schema_id_v1(&schemas, kind).expect("facet schema"),
                    fixture_facet_value_v1(kind, [kind.tag() as u8; 32], vec![[4; 32]]),
                )
            };
            CandidateContractComponentV1::new(
                &schemas,
                kind,
                schema_id,
                value,
                vec![],
                ComponentProvenanceV1::design_slot(
                    design_revision_id,
                    kind.tag(),
                    source_binding_id,
                )
                .expect("provenance"),
            )
            .expect("component")
        })
        .collect();
    CandidateContractRootV1::new(&schemas, components).expect("base root")
}

#[test]
fn consequence_plan_is_bound_to_an_exact_base_component_closure() {
    let root = base_root();
    let replaced = root
        .components()
        .iter()
        .find(|component| component.kind() == ContractComponentKindV1::IntendedOutcome)
        .expect("replaced component");
    let retained = root
        .components()
        .iter()
        .filter(|component| component.component_id() != replaced.component_id())
        .map(|component| *component.component_id())
        .collect::<Vec<_>>();
    let schemas = candidate_root_schema_closure_v1().expect("schema closure");
    let addition = PlannedContractComponentV1::new(
        PlannedContractSlotV1::new(1).expect("slot"),
        ContractComponentKindV1::IntendedOutcome,
        facet_schema_id_v1(&schemas, ContractComponentKindV1::IntendedOutcome)
            .expect("facet schema"),
        fixture_facet_value_v1(ContractComponentKindV1::IntendedOutcome, [99; 32], vec![]),
        Vec::<PlannedContractDependencyV1>::new(),
    )
    .expect("planned component");

    let plan = ContractConsequencePlanV1::new(7, &root, retained, vec![addition])
        .expect("exact consequence plan");

    assert_eq!(plan.base_root_id(), root.root_id());
    assert_eq!(
        plan.retained_component_ids().len(),
        root.components().len() - 1
    );
    assert_eq!(plan.additions().len(), 1);

    let unknown = ContractComponentIdV1::parse(&rendered(250)).expect("unknown component");
    assert!(ContractConsequencePlanV1::new(7, &root, vec![unknown], vec![]).is_err());
}
