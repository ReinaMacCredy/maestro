use maestro::domain::design::{
    AppendixRootV1, DesignAppendEligibilityV1, DesignClosureRequirementSnapshotV1,
    DesignReconciliationSnapshotV1, DesignSlotDispositionV1, DesignSlotEntryV1, DesignSlotIdV1,
    DesignSlotManifestV1, DesignSourceBindingV1, DesignSourceClassificationV1, DesignSourceKindV1,
    DesignStreamV1, DesignV1Error, ExactRecordRefV1, WorkIdV1,
};
use maestro::domain::identity::{ContractRootIdV1, DecisionClosureIdV1, StoreDomainIdV1};

fn exact(seed: u8) -> ExactRecordRefV1 {
    ExactRecordRefV1::from_digest([seed; 32])
}

fn contract_root(seed: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{seed:02x}").repeat(32)))
        .expect("Contract Root identity")
}

fn decision_closure(seed: u8) -> DecisionClosureIdV1 {
    DecisionClosureIdV1::parse(&format!("sha256:{}", format!("{seed:02x}").repeat(32)))
        .expect("Decision Closure identity")
}

fn repository() -> StoreDomainIdV1 {
    StoreDomainIdV1::parse(&format!("sha256:{}", "51".repeat(32))).expect("repository Store domain")
}

fn work() -> WorkIdV1 {
    WorkIdV1::derive("work-a").expect("Work identity")
}

fn source(slot: DesignSlotIdV1, bytes: &[u8]) -> DesignSourceBindingV1 {
    DesignSourceBindingV1::new(
        repository(),
        work(),
        slot,
        DesignSourceKindV1::Research,
        exact(11),
        exact(12),
        DesignSourceClassificationV1::Normative,
        bytes.to_vec(),
    )
    .expect("source binding")
}

#[test]
fn source_binding_identity_covers_exact_bytes_and_classification() {
    let slot = DesignSlotIdV1::new(1).expect("slot");
    let first = source(slot, b"exact source bytes");
    let identical = source(slot, b"exact source bytes");
    let changed = source(slot, b"changed source bytes");
    let contextual = DesignSourceBindingV1::new(
        repository(),
        work(),
        slot,
        DesignSourceKindV1::Research,
        exact(11),
        exact(12),
        DesignSourceClassificationV1::ContextOnly,
        b"exact source bytes".to_vec(),
    )
    .expect("context source binding");

    assert_eq!(first.binding_id(), identical.binding_id());
    assert_ne!(first.binding_id(), changed.binding_id());
    assert_ne!(first.binding_id(), contextual.binding_id());
    assert_eq!(first.source_bytes(), b"exact source bytes");
}

#[test]
fn revision_manifest_is_total_and_appendices_cannot_close_missing_slots() {
    let first_slot = DesignSlotIdV1::new(1).expect("slot");
    let second_slot = DesignSlotIdV1::new(2).expect("slot");
    let requirement = DesignClosureRequirementSnapshotV1::new(
        repository(),
        work(),
        vec![first_slot, second_slot],
    )
    .expect("closure requirement");
    let first_source = source(first_slot, b"first");

    let missing_entry = DesignSlotEntryV1::new(second_slot, DesignSlotDispositionV1::Missing);
    let manifest = DesignSlotManifestV1::new(
        &requirement,
        std::slice::from_ref(&first_source),
        vec![
            DesignSlotEntryV1::new(
                first_slot,
                DesignSlotDispositionV1::satisfied(vec![*first_source.binding_id()])
                    .expect("satisfied slot"),
            ),
            missing_entry,
        ],
    )
    .expect("total manifest with explicit missing disposition");
    let revision = maestro::domain::design::DesignRevisionV1::new(
        repository(),
        work(),
        None,
        &requirement,
        manifest,
        vec![first_source],
        vec![AppendixRootV1::new(exact(99))],
    )
    .expect("revision");
    let reconciliation = DesignReconciliationSnapshotV1::new(
        &revision,
        decision_closure(31),
        contract_root(32),
        true,
        true,
        true,
    )
    .reconcile();

    assert_eq!(reconciliation.missing_slots(), &[second_slot]);
    assert!(!reconciliation.is_clean());
    assert_eq!(
        reconciliation.finalization_inputs(),
        Err(DesignV1Error::ReconciliationNotClean)
    );
}

#[test]
fn design_stream_append_is_cas_guarded_immutable_and_terminal_closed() {
    let slot = DesignSlotIdV1::new(1).expect("slot");
    let requirement = DesignClosureRequirementSnapshotV1::new(repository(), work(), vec![slot])
        .expect("closure requirement");
    let first_source = source(slot, b"first");
    let first_manifest = DesignSlotManifestV1::new(
        &requirement,
        std::slice::from_ref(&first_source),
        vec![DesignSlotEntryV1::new(
            slot,
            DesignSlotDispositionV1::satisfied(vec![*first_source.binding_id()])
                .expect("satisfied"),
        )],
    )
    .expect("manifest");
    let genesis = maestro::domain::design::DesignRevisionV1::new(
        repository(),
        work(),
        None,
        &requirement,
        first_manifest,
        vec![first_source],
        vec![],
    )
    .expect("genesis");
    let stream = DesignStreamV1::new(genesis.clone()).expect("Design stream");

    let second_source = source(slot, b"second");
    let second_manifest = DesignSlotManifestV1::new(
        &requirement,
        std::slice::from_ref(&second_source),
        vec![DesignSlotEntryV1::new(
            slot,
            DesignSlotDispositionV1::satisfied(vec![*second_source.binding_id()])
                .expect("satisfied"),
        )],
    )
    .expect("manifest");
    let second = maestro::domain::design::DesignRevisionV1::new(
        repository(),
        work(),
        Some(*genesis.revision_id()),
        &requirement,
        second_manifest,
        vec![second_source],
        vec![],
    )
    .expect("second revision");
    let advanced = stream
        .append(
            genesis.revision_id(),
            second.clone(),
            DesignAppendEligibilityV1::Eligible,
        )
        .expect("CAS append");

    assert_eq!(stream.revisions(), std::slice::from_ref(&genesis));
    assert_eq!(advanced.revisions(), &[genesis.clone(), second.clone()]);
    assert_eq!(
        stream.append(
            second.revision_id(),
            second.clone(),
            DesignAppendEligibilityV1::Eligible,
        ),
        Err(DesignV1Error::StaleCandidateHead)
    );
    assert_eq!(
        stream.append(
            genesis.revision_id(),
            second,
            DesignAppendEligibilityV1::TerminalWork,
        ),
        Err(DesignV1Error::TerminalWorkRejectsDesignChange)
    );
}

#[test]
fn reconciliation_is_pure_and_only_clean_exact_snapshots_feed_finalization() {
    let slot = DesignSlotIdV1::new(1).expect("slot");
    let requirement = DesignClosureRequirementSnapshotV1::new(repository(), work(), vec![slot])
        .expect("closure requirement");
    let binding = source(slot, b"complete");
    let manifest = DesignSlotManifestV1::new(
        &requirement,
        std::slice::from_ref(&binding),
        vec![DesignSlotEntryV1::new(
            slot,
            DesignSlotDispositionV1::satisfied(vec![*binding.binding_id()]).expect("satisfied"),
        )],
    )
    .expect("manifest");
    let revision = maestro::domain::design::DesignRevisionV1::new(
        repository(),
        work(),
        None,
        &requirement,
        manifest,
        vec![binding],
        vec![],
    )
    .expect("revision");
    let snapshot = DesignReconciliationSnapshotV1::new(
        &revision,
        decision_closure(51),
        contract_root(52),
        true,
        true,
        true,
    );
    let report = snapshot.reconcile();
    let inputs = report
        .finalization_inputs()
        .expect("clean finalization inputs");

    assert!(report.is_clean());
    assert_eq!(inputs.design_revision_id(), revision.revision_id());
    assert_eq!(inputs.decision_closure_id(), &decision_closure(51));
    assert_eq!(inputs.candidate_contract_root_id(), &contract_root(52));
    assert_eq!(snapshot.reconcile(), report);

    let stale = DesignReconciliationSnapshotV1::new(
        &revision,
        decision_closure(51),
        contract_root(52),
        true,
        true,
        false,
    )
    .reconcile();
    assert!(!stale.is_clean());
    assert_eq!(
        stale.finalization_inputs(),
        Err(DesignV1Error::ReconciliationNotClean)
    );
}
