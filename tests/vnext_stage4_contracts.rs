use maestro::domain::execution::{WithdrawalDeniedProductV1, withdrawal_catalog_cells_v1};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static COMPILED_MUTANT_GATE: Mutex<()> = Mutex::new(());

type ManifestMutant = (&'static str, Box<dyn FnOnce(&mut Value)>);

fn canonical_model_row_mut<'a>(value: &'a mut Value, discriminator: &str) -> &'a mut Value {
    value["canonical_value"][5]
        .as_array_mut()
        .expect("Execution model rows")
        .iter_mut()
        .find(|row| row[0] == discriminator)
        .unwrap_or_else(|| panic!("missing Execution model row {discriminator}"))
}

struct TemporaryRoot(PathBuf);

impl TemporaryRoot {
    fn new(name: &str) -> Self {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maestro-stage4-proof-{}-{sequence}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary Stage 4 proof root");
        Self(path)
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn run_strings(repo: &Path, program: &str, args: &[String]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination tree");
    for entry in fs::read_dir(source).expect("read source tree") {
        let entry = entry.expect("read source entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy source file");
        }
    }
}

fn copy_file(repo: &Path, workspace: &Path, relative: &str) {
    let destination = workspace.join(relative);
    fs::create_dir_all(destination.parent().expect("destination parent"))
        .expect("create destination parent");
    fs::copy(repo.join(relative), destination).expect("copy workspace file");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON file")).expect("parse JSON file")
}

fn write_json(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("encode JSON file");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write JSON file");
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_proof_tree_drift_rejected(repo: &Path, expected: &Path, actual: &Path, label: &str) {
    let script = r#"import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location('stage4_build', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.compare_trees(pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3]))"#;
    let output = run_strings(
        repo,
        "python3",
        &[
            "-c".to_owned(),
            script.to_owned(),
            repo.join("tools/vnext_contracts/stage4/execution/build.py")
                .to_string_lossy()
                .into_owned(),
            expected.to_string_lossy().into_owned(),
            actual.to_string_lossy().into_owned(),
        ],
    );
    assert!(!output.status.success(), "Stage 4 compare accepted {label}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("proof receipt is stale"),
        "{label} failed outside receipt comparison\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn proof_receipt_rows(repo: &Path, paths: &[&str]) -> Value {
    Value::Array(
        paths
            .iter()
            .map(|relative| {
                let bytes = fs::read(repo.join(relative)).expect("read predecessor receipt");
                json!({
                    "byte_length": bytes.len(),
                    "path": relative,
                    "sha256": sha256_hex(&bytes),
                })
            })
            .collect(),
    )
}

fn validators() -> [(&'static str, &'static str); 2] {
    [
        (
            "python3",
            "tools/vnext_contracts/stage4/execution/validate.py",
        ),
        ("ruby", "tools/vnext_contracts/stage4/execution/verify.rb"),
    ]
}

fn assert_rejected_by_both(name: &str, mutate: impl FnOnce(&mut Value)) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new(name);
    let root = temporary.0.join("execution");
    copy_tree(&repo.join("contracts/vnext/stage4/execution"), &root);
    let manifest_path = root.join("execution-effects.v1.json");
    let mut manifest = read_json(&manifest_path);
    mutate(&mut manifest);
    write_json(&manifest_path, &manifest);
    let root_arg = root.to_str().expect("UTF-8 Stage 4 root");
    for (program, script) in validators() {
        let output = run(
            repo,
            program,
            &[script, "--root", root_arg, "--artifact-only"],
        );
        assert!(
            !output.status.success(),
            "{program} accepted Stage 4 mutant {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn source_paths(manifest: &Value) -> Vec<&str> {
    manifest["canonical_value"][9]
        .as_array()
        .expect("source rows")
        .iter()
        .map(|row| row[0].as_str().expect("source path"))
        .collect()
}

fn mutant_workspace(name: &str) -> (TemporaryRoot, PathBuf) {
    compiled_mutant_workspace(name)
}

fn replace_in_tree(root: &Path, from: &str, to: &str) -> usize {
    let mut replacements = 0;
    for entry in fs::read_dir(root).expect("read mutant source root") {
        let entry = entry.expect("read mutant source entry");
        let path = entry.path();
        if path.is_dir() {
            replacements += replace_in_tree(&path, from, to);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            let source = fs::read_to_string(&path).expect("read mutant Rust source");
            let count = source.matches(from).count();
            if count > 0 {
                fs::write(&path, source.replace(from, to)).expect("write mutant Rust source");
                replacements += count;
            }
        }
    }
    replacements
}

fn assert_workspace_rejected_by_both(workspace: &Path, name: &str) {
    for (program, script) in validators() {
        let output = run(workspace, program, &[script, "--source-only"]);
        assert!(
            !output.status.success(),
            "{program} accepted Stage 4 source mutant {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn stage4_certification_identity(workspace: &Path) -> String {
    read_json(
        &workspace.join("contracts/vnext/stage4/execution/execution-effects.v1.json"),
    )["identity"]
        .as_str()
        .expect("parent Stage 4 certification identity")
        .to_owned()
}

fn regenerate(workspace: &Path, root: &Path) -> Output {
    let parent_certification_identity = stage4_certification_identity(workspace);
    for (program, args) in [
        (
            "python3",
            &["tools/vnext_contracts/stage0/effect_home/build.py"][..],
        ),
        (
            "python3",
            &["tools/vnext_contracts/stage0/dispatch_cutover/build.py"][..],
        ),
        ("git", &["init", "--quiet"][..]),
        ("git", &["add", "-A"][..]),
        (
            "python3",
            &["tools/vnext_contracts/stage2/authority/build.py"][..],
        ),
        (
            "python3",
            &["tools/vnext_contracts/stage3/domain/build.py"][..],
        ),
    ] {
        let output = run(workspace, program, args);
        if !output.status.success() {
            return output;
        }
    }
    run_strings(
        workspace,
        "python3",
        &[
            "tools/vnext_contracts/stage4/execution/build.py".to_owned(),
            "--root".to_owned(),
            root.to_str()
                .expect("UTF-8 regenerated Stage 4 root")
                .to_owned(),
            "--skip-mutants".to_owned(),
            "--parent-certification-identity".to_owned(),
            parent_certification_identity,
        ],
    )
}

fn compiled_mutant_workspace(name: &str) -> (TemporaryRoot, PathBuf) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new(name);
    let workspace = temporary.0.join("workspace");
    fs::create_dir_all(&workspace).expect("create compiled mutant workspace");
    for relative in ["Cargo.toml", "Cargo.lock", "build.rs"] {
        copy_file(repo, &workspace, relative);
    }
    for relative in [
        "src",
        "embedded",
        "tests",
        "tools/vnext_contracts",
        "contracts/vnext",
    ] {
        copy_tree(&repo.join(relative), &workspace.join(relative));
    }
    (temporary, workspace)
}

#[cfg(any())]
mod superseded_callable_contracts {
    use super::*;

    fn execution_token(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn render_digest(value: [u8; 32]) -> String {
        let body = value
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("sha256:{body}")
    }

    fn execution_binding() -> StepBindingV1 {
        let repository = StoreDomainIdV1::from_digest(execution_token(1));
        let work = WorkIdV1::derive("stage4-work").expect("derive work identity");
        let scope = StepScopeV1::new(repository, work);
        StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest(execution_token(3)))
                .expect("parse contract generation"),
            ContractRootIdV1::from_digest(execution_token(4)),
            StepIdV1::from_bytes(scope, execution_token(5)).expect("derive Step identity"),
            StepRevisionIdV1::from_bytes(execution_token(6)).expect("derive Step revision"),
        )
        .expect("construct exact Step binding")
    }

    fn execution_authority(action: ExecutionActionV1, seed: &str) -> AuthorizedExecutionActionV1 {
        let request = CanonicalExecutionActionRequestV1::new(
            action,
            execution_token(41),
            execution_token(42),
            execution_token(43),
            IdempotencyKeyIdV1::derive(seed).expect("derive idempotency identity"),
        )
        .expect("construct canonical Execution Action Request");
        let receipt = AuthorizationReceiptV1::new(
            request.request_id(),
            AuthorityContextIdV1::derive("stage4-context").expect("derive Authority context"),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive("stage4-old-state").expect("derive prior State token"),
            StateTokenIdV1::derive("stage4-new-state").expect("derive resulting State token"),
        )
        .expect("construct Authorization Receipt");
        AuthorizedExecutionActionV1::new(request, receipt)
            .expect("bind authorization to Execution action")
    }

    fn execution_tenure() -> StepExecutionTenureV1 {
        StepExecutionTenureV1::acquire(StepExecutionAcquisitionV1 {
            binding: execution_binding(),
            next_fence: 7,
            executor: PrincipalIdV1::derive("stage4-executor").expect("derive executor"),
            store_generation_id: StoreGenerationIdV1::from_digest(execution_token(8)),
            authority_epoch: 9,
            fixed_envelope_commitment: execution_token(10),
            run_limit: 32,
            issued_at: 100,
            expires_at: 150,
            hard_deadline: 200,
            authority: execution_authority(
                ExecutionActionV1::AcquireStepExecution,
                "stage4-acquire",
            ),
        })
        .expect("acquire Step execution tenure")
    }

    fn execution_attempt(tenure: &StepExecutionTenureV1) -> ExecutionAttemptV1 {
        ExecutionAttemptV1::Step(tenure.attempt().clone())
    }

    fn run_reservation(tenure: &StepExecutionTenureV1, launch_ordinal: u32) -> RunReservationV1 {
        RunReservationV1 {
            semantic_operation_hash: execution_token(11),
            inputs_commitment: execution_token(12),
            environment_commitment: execution_token(13),
            target_commitment: execution_token(14),
            execution_boundary_commitment: execution_token(15),
            deadline: 180,
            launch_ordinal,
            current_step_term: Some(tenure.current_term().id()),
        }
    }

    #[test]
    fn stage4_callable_lease_attempt_pair_and_terms_are_exact() {
        let mut tenure = execution_tenure();
        assert_eq!(tenure.lease().attempt_id(), tenure.attempt().id());
        assert_eq!(tenure.attempt().lease_id(), tenure.lease().id());
        assert_eq!(tenure.lease().binding(), tenure.attempt().binding());
        assert_eq!(tenure.lease().fence(), tenure.attempt().fence());
        assert_eq!(tenure.terms().len(), 1);
        assert_eq!(tenure.current_term().ordinal(), 1);
        assert_eq!(tenure.current_term().prior_term_id(), None);

        let first = tenure.current_term().id();
        let second = tenure
            .renew(
                first,
                120,
                180,
                execution_authority(ExecutionActionV1::RenewStepLeaseTerm, "stage4-renew"),
            )
            .expect("renew the exact live Lease term");
        assert_eq!(second.ordinal(), 2);
        assert_eq!(second.prior_term_id(), Some(first));
        assert_eq!(tenure.lease().current_term_id(), second.id());
        assert_eq!(tenure.terms().len(), 2);

        assert_eq!(
            tenure.renew(
                first,
                130,
                190,
                execution_authority(ExecutionActionV1::RenewStepLeaseTerm, "stage4-stale-renew",),
            ),
            Err(ExecutionRuntimeErrorV1::StaleLeaseTerm)
        );
        let second = tenure.current_term().id();
        assert_eq!(
            tenure.abandon(
                StepAttemptTerminalV1::Submitted,
                second,
                130,
                execution_authority(
                    ExecutionActionV1::AbandonStepAttempt,
                    "stage4-submit-smuggle",
                ),
            ),
            Err(ExecutionRuntimeErrorV1::SubmissionOwnedByStep)
        );
        assert_eq!(tenure.attempt().state(), StepAttemptStateV1::Live);

        tenure
            .abandon(
                StepAttemptTerminalV1::Yielded,
                second,
                130,
                execution_authority(ExecutionActionV1::AbandonStepAttempt, "stage4-yield"),
            )
            .expect("close the Lease and Attempt atomically");
        let closed = StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Yielded);
        assert_eq!(tenure.attempt().state(), closed);
        assert_eq!(tenure.lease().state(), closed);
        assert_eq!(
            tenure.renew(
                second,
                140,
                190,
                execution_authority(
                    ExecutionActionV1::RenewStepLeaseTerm,
                    "stage4-terminal-renew",
                ),
            ),
            Err(ExecutionRuntimeErrorV1::TerminalAttempt)
        );
    }

    #[test]
    fn stage4_callable_run_graph_is_exact_and_owner_fenced() {
        let reserved_successors = [
            RunStateV1::Active,
            RunStateV1::DefinitelyNotStarted,
            RunStateV1::Cancelled,
            RunStateV1::TimedOut,
            RunStateV1::Lost,
            RunStateV1::Fenced,
        ];
        for next in reserved_successors {
            let tenure = execution_tenure();
            let attempt = execution_attempt(&tenure);
            let mut run = RunV1::reserve(&attempt, run_reservation(&tenure, 1))
                .expect("reserve Step-owned Run");
            assert_eq!(
                run.owner(),
                ExecutionAttemptOwnerV1::Step(tenure.attempt().id())
            );
            run.transition(next)
                .unwrap_or_else(|error| panic!("Reserved -> {next:?} was rejected: {error}"));
            assert_eq!(run.state(), next);
        }

        for next in [
            RunStateV1::Reserved,
            RunStateV1::Succeeded,
            RunStateV1::Failed,
        ] {
            let tenure = execution_tenure();
            let attempt = execution_attempt(&tenure);
            let mut run = RunV1::reserve(&attempt, run_reservation(&tenure, 1))
                .expect("reserve Step-owned Run");
            assert_eq!(
                run.transition(next),
                Err(ExecutionRuntimeErrorV1::IllegalRunTransition),
                "Reserved -> {next:?} must remain illegal"
            );
        }

        let active_successors = [
            RunStateV1::Succeeded,
            RunStateV1::Failed,
            RunStateV1::Cancelled,
            RunStateV1::TimedOut,
            RunStateV1::Lost,
            RunStateV1::Fenced,
        ];
        for next in active_successors {
            let tenure = execution_tenure();
            let attempt = execution_attempt(&tenure);
            let mut run = RunV1::reserve(&attempt, run_reservation(&tenure, 1))
                .expect("reserve Step-owned Run");
            run.transition(RunStateV1::Active)
                .expect("activate reserved Run");
            run.transition(next)
                .unwrap_or_else(|error| panic!("Active -> {next:?} was rejected: {error}"));
            assert_eq!(run.state(), next);
            assert_eq!(
                run.transition(RunStateV1::Active),
                Err(ExecutionRuntimeErrorV1::IllegalRunTransition),
                "terminal {next:?} Run reopened"
            );
        }

        for next in [
            RunStateV1::Reserved,
            RunStateV1::Active,
            RunStateV1::DefinitelyNotStarted,
        ] {
            let tenure = execution_tenure();
            let attempt = execution_attempt(&tenure);
            let mut run = RunV1::reserve(&attempt, run_reservation(&tenure, 1))
                .expect("reserve Step-owned Run");
            run.transition(RunStateV1::Active)
                .expect("activate reserved Run");
            assert_eq!(
                run.transition(next),
                Err(ExecutionRuntimeErrorV1::IllegalRunTransition),
                "Active -> {next:?} must remain illegal"
            );
        }

        let tenure = execution_tenure();
        let attempt = execution_attempt(&tenure);
        let mut missing_term = run_reservation(&tenure, 1);
        missing_term.current_step_term = None;
        assert_eq!(
            RunV1::reserve(&attempt, missing_term),
            Err(ExecutionRuntimeErrorV1::StepRunRequiresLiveTerm)
        );

        let mut run =
            RunV1::reserve(&attempt, run_reservation(&tenure, 1)).expect("reserve retry probe Run");
        assert_eq!(
            run.retry_reservation(&attempt, 190),
            Err(ExecutionRuntimeErrorV1::RunRetryBoundaryUnknown)
        );
        run.transition(RunStateV1::DefinitelyNotStarted)
            .expect("prove Run definitely did not start");
        let retry = run
            .retry_reservation(&attempt, 190)
            .expect("derive same-Attempt relaunch reservation");
        assert_eq!(retry.launch_ordinal, 2);
        assert_eq!(retry.semantic_operation_hash, execution_token(11));
        assert_eq!(retry.current_step_term, Some(tenure.current_term().id()));
    }

    #[test]
    fn stage4_callable_run_set_cas_and_submission_fence_are_atomic() {
        let tenure = execution_tenure();
        let attempt = execution_attempt(&tenure);
        let run =
            RunV1::reserve(&attempt, run_reservation(&tenure, 1)).expect("reserve Run-set member");
        let run_id = run.id();
        let mut run_set = RunSetV1::new(&attempt);
        assert_eq!(run_set.revision(), 1);
        run_set.insert(run).expect("insert exact-owner Run");
        assert_eq!(run_set.revision(), 2);
        assert_eq!(
            run_set.transition(run_id, 1, RunStateV1::Active),
            Err(ExecutionRuntimeErrorV1::StaleRunSetRevision)
        );
        assert_eq!(run_set.revision(), 2);
        run_set
            .transition(run_id, 2, RunStateV1::Active)
            .expect("CAS transition exact Run-set revision");
        assert_eq!(run_set.revision(), 3);
        assert_eq!(
            tenure.submission_fence(tenure.current_term().id(), 120, &run_set),
            Err(ExecutionRuntimeErrorV1::SubmissionFenceUnavailable)
        );
        run_set
            .transition(run_id, 3, RunStateV1::Succeeded)
            .expect("finish Run before Step submission");
        let fence = tenure
            .submission_fence(tenure.current_term().id(), 120, &run_set)
            .expect("issue quiescent Step submission fence");
        assert_eq!(fence.attempt_id(), tenure.attempt().id());
        assert_eq!(fence.term_id(), tenure.current_term().id());
        assert_eq!(fence.run_set_revision(), 4);
    }
}

#[test]
fn stage4_public_effect_facade_exports_are_complete() {
    assert_external_io_release_capabilities_are_non_clone_and_non_extractable();
    use maestro::domain::evidence::SubmissionClaimSetV1;
    use maestro::domain::execution::{
        ActiveStoreEffectOriginationDraftV1, ActiveStoreEffectOriginationOutcomeV1,
        ActiveStoreEffectOriginationPublicationV1, ActiveStoreEffectReconciliationBeginDraftV1,
        ActiveStoreEffectReconciliationBeginPublicationV1,
        ActiveStoreEffectReconciliationOutcomeV1, ActiveStoreEffectReconciliationReadDraftV1,
        ActiveStoreEffectReconciliationReadPublicationV1,
        ActiveStoreEffectReconciliationTerminalDraftV1,
        ActiveStoreEffectReconciliationTerminalPublicationV1,
        ActiveStoreEffectRecoverReservedDraftV1, ActiveStoreEffectRecoverReservedOutcomeV1,
        ActiveStoreEffectRecoverReservedPublicationV1, ActiveStoreEffectRedispatchDraftV1,
        ActiveStoreEffectRedispatchOutcomeV1, ActiveStoreEffectRedispatchPublicationV1,
        ActiveStoreEffectSealDraftV1, ActiveStoreEffectSealOutcomeV1,
        ActiveStoreEffectSealPublicationV1, ActiveStoreEffectSnapshotV1,
        ActiveStoreEffectTerminalDraftV1, ActiveStoreEffectTerminalOutcomeV1,
        ActiveStoreEffectTerminalPublicationV1, ActiveStoreEffectWithdrawalDraftV1,
        ActiveStoreEffectWithdrawalOutcomeV1, ActiveStoreEffectWithdrawalPublicationV1,
        ActiveStoreEffectWriterHandoffDraftV1, ActiveStoreEffectWriterHandoffOutcomeV1,
        ActiveStoreEffectWriterHandoffPublicationV1, CeremonyRequestModeV1, CeremonySpecV1,
        EffectCredentialRequirementsV1, EffectDispatchBindingInputsV1,
        EffectDispatchOutcomePayloadV1, EffectMaterialInputsV1, EffectOriginKindV1, EffectOriginV1,
        EffectReconciliationAttemptV1, EffectReconciliationOutcomeV1,
        EffectReconciliationPreparationV1, EffectReconciliationReadPlanPartsV1,
        EffectReconciliationReadPlanV1, EffectReconciliationReadUsageV1, EffectRuntimeErrorV1,
        EffectSemanticUseV1, ExecutionStoreFacadeV1, HomeTokenV1, ProtectedCeremonyAuthorityV1,
        ProtectedCeremonyCarrierAnchorV1, ProtectedCeremonyEffectCarrierV1,
        ProtectedCeremonyEffectErrorV1, ProtectedCeremonyEffectOutcomeV1,
        ProtectedCeremonyEffectPhaseV1, ProtectedCeremonyEffectRequestV1,
        ProtectedCeremonyEffectStoreV1, ProtectedCeremonyOwnerAuthorityV1,
        ProviderApplicationReleaseV1, ReconciliationReadOperationClassificationV1,
        ReconciliationReadOperationKindV1, RunExecutionTimeReceiptV1, RunNoStartReceiptV1,
    };

    fn public_contract<T: Send + Sync>() {}
    fn public_store_facade<T: Send>() {}

    public_contract::<ActiveStoreEffectOriginationDraftV1>();
    public_contract::<ActiveStoreEffectOriginationPublicationV1>();
    public_contract::<ActiveStoreEffectOriginationOutcomeV1>();
    public_contract::<ActiveStoreEffectRedispatchDraftV1>();
    public_contract::<ActiveStoreEffectRedispatchPublicationV1>();
    public_contract::<ActiveStoreEffectRedispatchOutcomeV1>();
    public_contract::<ActiveStoreEffectRecoverReservedDraftV1>();
    public_contract::<ActiveStoreEffectRecoverReservedPublicationV1>();
    public_contract::<ActiveStoreEffectRecoverReservedOutcomeV1>();
    public_contract::<ActiveStoreEffectSealDraftV1>();
    public_contract::<ActiveStoreEffectSealPublicationV1>();
    public_contract::<ActiveStoreEffectSealOutcomeV1>();
    public_contract::<ActiveStoreEffectTerminalDraftV1>();
    public_contract::<ActiveStoreEffectTerminalPublicationV1>();
    public_contract::<ActiveStoreEffectTerminalOutcomeV1>();
    public_contract::<ActiveStoreEffectSnapshotV1>();
    public_contract::<ActiveStoreEffectReconciliationBeginDraftV1>();
    public_contract::<ActiveStoreEffectReconciliationBeginPublicationV1>();
    public_contract::<ActiveStoreEffectReconciliationReadDraftV1>();
    public_contract::<ActiveStoreEffectReconciliationReadPublicationV1>();
    public_contract::<ActiveStoreEffectReconciliationTerminalDraftV1>();
    public_contract::<ActiveStoreEffectReconciliationTerminalPublicationV1>();
    public_contract::<ActiveStoreEffectReconciliationOutcomeV1>();
    public_contract::<ActiveStoreEffectWithdrawalDraftV1>();
    public_contract::<ActiveStoreEffectWithdrawalPublicationV1>();
    public_contract::<ActiveStoreEffectWithdrawalOutcomeV1>();
    public_contract::<ActiveStoreEffectWriterHandoffDraftV1>();
    public_contract::<ActiveStoreEffectWriterHandoffPublicationV1>();
    public_contract::<ActiveStoreEffectWriterHandoffOutcomeV1>();
    public_contract::<CeremonySpecV1>();
    public_contract::<ProtectedCeremonyAuthorityV1>();
    public_contract::<ProtectedCeremonyEffectRequestV1>();
    public_contract::<ProtectedCeremonyEffectCarrierV1>();
    public_contract::<ProtectedCeremonyEffectOutcomeV1>();
    public_contract::<ProtectedCeremonyEffectErrorV1>();
    public_contract::<ProtectedCeremonyEffectStoreV1>();
    public_contract::<ProtectedCeremonyOwnerAuthorityV1>();
    public_contract::<ProtectedCeremonyCarrierAnchorV1>();
    public_contract::<ProtectedCeremonyEffectPhaseV1>();
    public_contract::<SubmissionClaimSetV1>();
    public_contract::<ProviderApplicationReleaseV1>();
    public_contract::<RunExecutionTimeReceiptV1>();
    public_contract::<RunNoStartReceiptV1>();
    public_contract::<EffectCredentialRequirementsV1>();
    public_contract::<EffectDispatchBindingInputsV1>();
    public_contract::<EffectDispatchOutcomePayloadV1>();
    public_contract::<EffectMaterialInputsV1>();
    public_contract::<EffectOriginKindV1>();
    public_contract::<EffectOriginV1>();
    public_contract::<EffectReconciliationAttemptV1>();
    public_contract::<EffectReconciliationOutcomeV1>();
    public_contract::<EffectReconciliationPreparationV1>();
    public_contract::<EffectReconciliationReadPlanPartsV1>();
    public_contract::<EffectReconciliationReadPlanV1>();
    public_contract::<EffectReconciliationReadUsageV1>();
    public_contract::<EffectRuntimeErrorV1>();
    public_contract::<ReconciliationReadOperationClassificationV1>();
    public_contract::<ReconciliationReadOperationKindV1>();
    public_contract::<EffectSemanticUseV1>();
    public_store_facade::<ExecutionStoreFacadeV1<'static>>();

    let temporary = TemporaryRoot::new("public-ceremony-owner-issuer");
    let managed_root = fs::canonicalize(&temporary.0).expect("canonical managed Ceremony root");
    let spec = CeremonySpecV1::InstallationContextGenesis;
    let incarnation = HomeTokenV1::new([41; 32]);
    let owner_basis = HomeTokenV1::new([42; 32]);
    let owner = ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(owner_basis)
        .expect("construct external owner authority");
    let (store, anchor) =
        ProtectedCeremonyEffectStoreV1::initialize(&managed_root, spec, incarnation, &owner)
            .expect("initialize owner-scoped Ceremony store");
    let empty = store.current().expect("load Ceremony genesis");
    let request = owner
        .issue_request(
            &store,
            CeremonyRequestModeV1::Initiate,
            empty.current_token(),
            HomeTokenV1::new([43; 32]),
            HomeTokenV1::new([44; 32]),
        )
        .expect("owner issues exact one-use Ceremony authority");
    let outcome = store
        .publish(request)
        .expect("publish owner-issued Ceremony");
    assert!(matches!(
        outcome.phase(),
        ProtectedCeremonyEffectPhaseV1::Reserved { .. }
    ));
    assert_eq!(outcome.provider_io_operations(), 0);
    let reopened = ProtectedCeremonyEffectStoreV1::open(managed_root, &anchor, &owner)
        .expect("reopen canonical Ceremony store");
    assert_eq!(
        reopened.current().expect("reload Ceremony").current_token(),
        outcome.current_token()
    );
}

fn assert_external_io_release_capabilities_are_non_clone_and_non_extractable() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(repo.join("src/domain/execution/store.rs"))
        .expect("read Execution Store source");
    for start in [
        "pub struct SealedProviderOperationV1",
        "pub struct SealedReconciliationReadV1",
    ] {
        let struct_start = source.find(start).expect("sealed capability declaration");
        let start = source[..struct_start]
            .rfind("#[derive(")
            .expect("sealed capability derive");
        let end = struct_start
            + source[struct_start..]
                .find("\n}\n")
                .expect("sealed capability declaration boundary")
            + 3;
        let declaration = &source[start..end];
        assert!(!declaration.contains("derive(Clone"));
        assert!(!declaration.contains("derive(Copy"));
    }
    assert!(!source.contains("pub const fn operation(&self)"));
    assert!(!source.contains("pub const fn read(&self)"));
    assert!(
        source
            .matches("execution_time: RunExecutionTimeReceiptV1")
            .count()
            >= 2
    );
}

#[test]
fn stage4_regenerated_same_name_behavior_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("same-name-run-behavior");
    let runtime_path = workspace.join("src/domain/execution/runtime.rs");
    let source = fs::read_to_string(&runtime_path).expect("read copied Execution runtime");
    let legal_reserved_tail =
        "RunStateV1::DefinitelyNotStarted\n                    | RunStateV1::Cancelled";
    let mutant_reserved_tail = "RunStateV1::Succeeded\n                    | RunStateV1::DefinitelyNotStarted\n                    | RunStateV1::Cancelled";
    assert_eq!(
        source.matches(legal_reserved_tail).count(),
        1,
        "the compiled mutant must alter exactly the live Reserved transition arm"
    );
    fs::write(
        &runtime_path,
        source.replacen(legal_reserved_tail, mutant_reserved_tail, 1),
    )
    .expect("write same-name callable behavior mutant");

    let regenerated = workspace.join("proof/execution");
    let regenerated_output = regenerate(&workspace, &regenerated);
    assert!(
        !regenerated_output.status.success(),
        "the Stage 4 build gate accepted a same-name Run behavior mutant\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&regenerated_output.stdout),
        String::from_utf8_lossy(&regenerated_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&regenerated_output.stdout)
            .contains("Reserved -> Succeeded must remain illegal")
            || String::from_utf8_lossy(&regenerated_output.stderr)
                .contains("Reserved -> Succeeded must remain illegal"),
        "same-name behavior mutant failed outside the Stage 4 compiled certification\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&regenerated_output.stdout),
        String::from_utf8_lossy(&regenerated_output.stderr)
    );
}

#[test]
fn stage4_regenerated_public_facade_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("missing-public-effect-facade");
    let facade_path = workspace.join("src/domain/execution/mod.rs");
    let source = fs::read_to_string(&facade_path).expect("read copied Execution facade");
    let public_export = "    ActiveStoreEffectReconciliationOutcomeV1, ActiveStoreEffectReconciliationReadDraftV1,\n";
    assert_eq!(
        source.matches(public_export).count(),
        1,
        "the public-facade mutant must remove exactly one reconciliation export"
    );
    fs::write(
        &facade_path,
        source.replacen(
            public_export,
            "    ActiveStoreEffectReconciliationReadDraftV1,\n",
            1,
        ),
    )
    .expect("write missing public-facade mutant");

    let regenerated = workspace.join("proof/execution");
    let output = regenerate(&workspace, &regenerated);
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a missing public effect facade\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        diagnostics.contains("ActiveStoreEffectReconciliationOutcomeV1"),
        "public-facade mutant failed outside the compiled public certification\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_basis_donation_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("execution-basis-donation");
    let authority_path = workspace.join("src/domain/authority/facade/repository_leaf_authority.rs");
    let source = fs::read_to_string(&authority_path).expect("read copied authority facade");
    let exact_guard = "if action.execution_authority_basis()
            != Some(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime)
        {";
    assert_eq!(source.matches(exact_guard).count(), 1);
    fs::write(
        &authority_path,
        source.replacen(
            exact_guard,
            "if action.execution_authority_basis().is_none() {",
            1,
        ),
    )
    .expect("write basis-donation mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted ordinary authority for specialized leaves"
    );
    assert!(
        diagnostics.contains("specialized_effect_authority_rejects_cross_basis"),
        "basis-donation mutant failed outside the compiled specialized-authority proof\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_ceremony_replay_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("ceremony-replay");
    let ceremony_path = workspace.join("src/domain/execution/ceremony.rs");
    let source = fs::read_to_string(&ceremony_path).expect("read copied Ceremony carrier");
    let exact_replay = r#"        if let Some(replay) = load_idempotency_outcome(
            &transaction,
            request.authority.idempotency_key,
            meaning,
            request.id,
        )? {"#;
    let skipped_replay = r#"        if let Some(replay) = if false {
            load_idempotency_outcome(
                &transaction,
                request.authority.idempotency_key,
                meaning,
                request.id,
            )?
        } else {
            None
        } {"#;
    assert_eq!(source.matches(exact_replay).count(), 1);
    fs::write(
        &ceremony_path,
        source.replacen(exact_replay, skipped_replay, 1),
    )
    .expect("write Ceremony replay mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a Ceremony carrier without exact replay"
    );
    assert!(
        diagnostics.contains(
            "protected_ceremony_has_one_winner_durable_full_history_replay_and_owner_refusal"
        ),
        "Ceremony mutant failed outside the durable protected-CAS proof\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_ceremony_descriptor_binding_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("ceremony-descriptor-binding");
    let ceremony_path = workspace.join("src/domain/execution/ceremony.rs");
    let source = fs::read_to_string(&ceremony_path).expect("read copied Ceremony carrier");
    let binding_start = source
        .find("#[cfg(unix)]\nfn verify_connection_leaf")
        .expect("Unix Ceremony descriptor binding");
    let (prefix, binding) = source.split_at(binding_start);
    let exact_guard = "|| metadata_identity(&metadata) != expected";
    assert!(binding.contains(exact_guard));
    fs::write(
        &ceremony_path,
        format!(
            "{prefix}{}",
            binding.replacen(exact_guard, "|| { let _ = expected; false }", 1)
        ),
    )
    .expect("write Ceremony descriptor-binding mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a Ceremony connection not bound to the anchored inode"
    );
    assert!(
        diagnostics.contains(
            "protected_ceremony_reopens_from_durable_anchor_and_rejects_inode_replacement"
        ),
        "Ceremony descriptor mutant failed outside the anchored-inode proof\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_writer_health_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("writer-health");
    let control_path = workspace.join("src/domain/execution/control_head.rs");
    let source = fs::read_to_string(&control_path).expect("read copied control Head");
    let exact_guard = "if current.health() == EffectIntentControlHealthV1::IntegrityBlocked =>";
    assert_eq!(source.matches(exact_guard).count(), 1);
    fs::write(
        &control_path,
        source.replacen(
            exact_guard,
            "if current.health() == EffectIntentControlHealthV1::Healthy =>",
            1,
        ),
    )
    .expect("write writer-health mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted an integrity-blocked writer handoff"
    );
    assert!(
        diagnostics.contains("unhealthy_control_products_fail_closed_until_exact_recovery"),
        "writer-health mutant failed outside the fail-closed Head proof\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_evidence_claim_set_owner_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("claim-set-owner");
    let evidence_path = workspace.join("src/domain/evidence/mod.rs");
    let contract_path = workspace.join("src/domain/contract/mod.rs");
    let evidence_source = fs::read_to_string(&evidence_path).expect("read copied Evidence facade");
    assert_eq!(
        evidence_source.matches("pub mod submission_claim;").count(),
        1
    );
    fs::write(
        &evidence_path,
        evidence_source.replacen(
            "pub mod submission_claim;",
            "pub use crate::domain::contract::submission_claim;",
            1,
        ),
    )
    .expect("write Contract-backed Evidence ClaimSet re-export mutant");
    let contract_source = fs::read_to_string(&contract_path).expect("read copied Contract facade");
    fs::write(
        &contract_path,
        contract_source.replacen(
            "pub mod runtime;",
            "pub mod runtime;\npub mod submission_claim;",
            1,
        ),
    )
    .expect("write Contract ClaimSet owner mutant");
    fs::rename(
        workspace.join("src/domain/evidence/submission_claim.rs"),
        workspace.join("src/domain/contract/submission_claim.rs"),
    )
    .expect("relocate ClaimSet definition under Contract");
    for relative in [
        "tools/vnext_contracts/stage3/domain/build.py",
        "tools/vnext_contracts/stage3/domain/validate.py",
        "tools/vnext_contracts/stage3/domain/verify.rb",
    ] {
        let path = workspace.join(relative);
        let source = fs::read_to_string(&path).expect("read copied Stage 3 proof source");
        assert_eq!(
            source
                .matches("src/domain/evidence/submission_claim.rs")
                .count(),
            1
        );
        fs::write(
            path,
            source.replacen(
                "src/domain/evidence/submission_claim.rs",
                "src/domain/contract/submission_claim.rs",
                1,
            ),
        )
        .expect("retarget copied Stage 3 proof source to the mutant owner");
    }

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted Contract-owned ClaimSet semantics"
    );
    assert!(
        diagnostics.contains("SubmissionClaimSetV1"),
        "ClaimSet-owner mutant failed outside the compiled Evidence facade proof\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_claim_set_digest_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("claim-set-digest");
    let submission_path = workspace.join("src/domain/step/submission.rs");
    let source = fs::read_to_string(&submission_path).expect("read copied Step Submission");
    let exact_digest = "            *claim_set.digest(),";
    assert_eq!(source.matches(exact_digest).count(), 1);
    fs::write(
        &submission_path,
        source.replacen(exact_digest, "            [255; 32],", 1),
    )
    .expect("write changed ClaimSet digest mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a changed stored ClaimSet digest"
    );
    assert!(
        diagnostics.contains("step_submission"),
        "ClaimSet-digest mutant failed outside compiled Submission behavior\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_non_atomic_claim_set_participant_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("non-atomic-claim-set");
    let store_path = workspace.join("src/domain/execution/store.rs");
    let source = fs::read_to_string(&store_path).expect("read copied Execution Store");
    let atomic_participants =
        "        submission_object,\n        claim_set_object,\n        next_index,";
    assert_eq!(source.matches(atomic_participants).count(), 1);
    fs::write(
        &store_path,
        source.replacen(
            atomic_participants,
            "        submission_object,\n        next_index,",
            1,
        ),
    )
    .expect("write missing atomic ClaimSet participant mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a split Submission participant generation"
    );
    assert!(
        diagnostics.contains("step_submission") || diagnostics.contains("claim_set_object"),
        "atomic-participant mutant failed outside compiled Submission behavior\n{diagnostics}"
    );
}

#[test]
fn stage4_regenerated_split_step_submission_generation_mutant_fails_compiled_contract() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("split-submission-generation");
    let store_path = workspace.join("src/domain/execution/store.rs");
    let source = fs::read_to_string(&store_path).expect("read copied Execution Store");
    let exact_successor = "current_generation\n            .ordinal()\n            .checked_add(1)";
    let function_start = source
        .find("fn build_step_submission_authorized_publication(")
        .expect("locate Step Submission publication builder");
    let function_end = function_start
        + source[function_start..]
            .find("\nfn execution_action_request_object(")
            .expect("locate end of Step Submission publication builder");
    let function = &source[function_start..function_end];
    assert_eq!(function.matches(exact_successor).count(), 1);
    let mutated_function = function.replacen(
        exact_successor,
        "current_generation\n            .ordinal()\n            .checked_add(2)",
        1,
    );
    fs::write(
        &store_path,
        format!(
            "{}{}{}",
            &source[..function_start],
            mutated_function,
            &source[function_end..]
        ),
    )
    .expect("write split Step Submission generation mutant");

    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "the Stage 4 build gate accepted a non-successor Submission generation"
    );
    assert!(
        diagnostics.contains("step_submission_one_and_many_claims_are_atomic"),
        "split-generation mutant failed outside the compiled atomic Submission proof\n{diagnostics}"
    );
}

#[test]
fn stage4_behavior_receipt_binds_the_compiled_execution_gate() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest =
        read_json(&repo.join("contracts/vnext/stage4/execution/execution-effects.v1.json"));
    let receipt =
        read_json(&repo.join("contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json"));
    assert_eq!(receipt["identity"], manifest["identity"]);
    assert_eq!(receipt["result"], "pass");
    assert_eq!(receipt["validation_mode"], "full_chain");
    assert_eq!(receipt["mutant_validation"], "executed");
    assert_eq!(
        receipt["commands"],
        json!([
            [
                "cargo",
                "test",
                "--lib",
                "domain::vnext::execution::",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--lib",
                "domain::vnext::authority::facade::repository_admission::ancestry_tests",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--lib",
                "domain::vnext::authority::continuity::trusted_time::tests",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--test",
                "vnext_stage4_contracts",
                "stage4_public_effect_facade_exports_are_complete",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--test",
                "vnext_stage4_contracts",
                "runtime_withdrawal_catalog_matches_all_sixty_frozen_rows_and_twenty_one_denials",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--test",
                "vnext_effect_home_literals",
                "stage0_effect_home_artifacts_are_reproducible_and_reject_mutants",
                "--",
                "--nocapture"
            ]
        ])
    );
    assert_eq!(
        receipt["mutant_commands"],
        json!([
            [
                "cargo",
                "test",
                "--test",
                "vnext_stage4_contracts",
                "stage4_regenerated_",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--test",
                "vnext_stage4_contracts",
                "stage4_proof_rejects_",
                "--",
                "--nocapture"
            ],
            [
                "cargo",
                "test",
                "--test",
                "vnext_stage4_contracts",
                "independent_execution_artifact_rejects_semantic_and_shape_mutants",
                "--",
                "--nocapture"
            ]
        ])
    );
    for (key, expected) in [
        ("command_receipts", [70, 7, 1, 1, 1, 1].as_slice()),
        ("mutant_command_receipts", [10, 6, 1].as_slice()),
    ] {
        let receipts = receipt[key].as_array().expect("execution receipts");
        assert_eq!(receipts.len(), expected.len());
        for (row, passed) in receipts.iter().zip(expected) {
            assert_eq!(row["exit_code"], 0);
            assert_eq!(row["result"], "pass");
            assert_eq!(row["passed"], *passed);
            assert_eq!(row["ignored"], 0);
            assert_eq!(
                row["test_names"]
                    .as_array()
                    .expect("exact test names")
                    .len(),
                *passed as usize
            );
            assert_eq!(
                row["test_binary"]["sha256"]
                    .as_str()
                    .expect("compiled test binary digest")
                    .len(),
                64
            );
            assert_eq!(
                row["normalized_output_sha256"]
                    .as_str()
                    .expect("normalized test output digest")
                    .len(),
                64
            );
            assert!(row["executable"]["invocation_path"].is_string());
            assert!(row["executable"]["resolved_path"].is_string());
        }
    }
}

#[test]
fn stage4_receipts_bind_the_exact_full_predecessor_chain() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stage0 =
        read_json(&repo.join("contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"));
    let stage2 =
        read_json(&repo.join("contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json"));
    let stage3 = read_json(&repo.join("contracts/vnext/stage3/domain/domain-kernel.v1.json"));
    let dispatch_path = "contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json";
    let expected_commands = json!([
        [
            "python3",
            "tools/vnext_contracts/stage0/effect_home/build.py",
            "--check"
        ],
        [
            "python3",
            "tools/vnext_contracts/stage0/effect_home/validate.py",
            "--mutants"
        ],
        [
            "python3",
            "tools/vnext_contracts/stage0/dispatch_cutover/build.py",
            "--check"
        ],
        [
            "python3",
            "tools/vnext_contracts/stage0/dispatch_cutover/validate.py",
            "--mutant-suite",
            "--no-write"
        ],
        [
            "python3",
            "tools/vnext_contracts/stage2/authority/build.py",
            "--check"
        ],
        [
            "python3",
            "tools/vnext_contracts/stage3/domain/build.py",
            "--check"
        ],
    ]);
    let expected_roots = json!({
        "stage0_effect_home": stage0["identity"],
        "stage0_dispatch_cutover": format!("sha256:{}", sha256_hex(&fs::read(repo.join(dispatch_path)).expect("dispatch receipt"))),
        "stage2_authority": format!("sha256:{}", stage2["root_id"].as_str().expect("Stage 2 root")),
        "stage3_domain": stage3["identity"],
    });
    let expected_proof_receipts = json!({
            "stage0_effect_home": proof_receipt_rows(repo, &[
                "contracts/vnext/stage0/effect-home/encoder-receipt.json",
                "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
            ]),
            "stage0_dispatch_cutover": proof_receipt_rows(repo, &[
                "contracts/vnext/stage0/dispatch-cutover/build-receipt.v1.json",
                dispatch_path,
            ]),
            "stage2_authority": proof_receipt_rows(repo, &[
                "contracts/vnext/stage2/authority/python-encoder-receipt.v1.json",
                "contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json",
                "contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json",
            ]),
            "stage3_domain": proof_receipt_rows(repo, &[
                "contracts/vnext/stage3/domain/python-encoder-receipt.v1.json",
                "contracts/vnext/stage3/domain/semantic-validation-receipt.v1.json",
                "contracts/vnext/stage3/domain/ruby-verification-receipt.v1.json",
            ]),
    });
    let mut exact_chain = None;
    for relative in [
        "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json",
        "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json",
        "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json",
    ] {
        let receipt = read_json(&repo.join(relative));
        assert_eq!(receipt["validation_mode"], "full_chain", "{relative}");
        let chain = &receipt["predecessor_chain"];
        assert_eq!(chain["mode"], "full_chain", "{relative}");
        assert_eq!(chain["roots"], expected_roots, "{relative}");
        assert_eq!(
            chain["proof_receipts"], expected_proof_receipts,
            "{relative}"
        );
        let command_receipts = chain["command_receipts"]
            .as_array()
            .expect("predecessor command receipts");
        assert_eq!(
            command_receipts.len(),
            expected_commands.as_array().unwrap().len()
        );
        for (row, command) in command_receipts
            .iter()
            .zip(expected_commands.as_array().unwrap())
        {
            assert_eq!(&row["command"], command);
            assert_eq!(row["exit_code"], 0);
            assert_eq!(row["result"], "pass");
            assert_eq!(row["stdout_sha256"].as_str().unwrap().len(), 64);
            assert_eq!(row["stderr_sha256"].as_str().unwrap().len(), 64);
            assert_eq!(row["executable"]["sha256"].as_str().unwrap().len(), 64);
        }
        if let Some(expected) = &exact_chain {
            assert_eq!(chain, expected, "{relative}");
        } else {
            exact_chain = Some(chain.clone());
        }
        if relative.ends_with("semantic-validation-receipt.v1.json")
            || relative.ends_with("ruby-verification-receipt.v1.json")
        {
            let reexecution = &receipt["behavioral_reexecution"];
            for (key, expected) in [
                ("command_receipts", [70, 7, 1, 1, 1, 1].as_slice()),
                ("mutant_command_receipts", [10, 6, 1].as_slice()),
            ] {
                let rows = reexecution[key]
                    .as_array()
                    .expect("independent compiled reexecution receipts");
                assert_eq!(rows.len(), expected.len());
                for (row, passed) in rows.iter().zip(expected) {
                    assert_eq!(row["passed"], *passed);
                    assert_eq!(row["ignored"], 0);
                    assert_eq!(
                        row["test_names"]
                            .as_array()
                            .expect("exact test names")
                            .len(),
                        *passed as usize
                    );
                    assert_eq!(row["test_binary"]["sha256"].as_str().unwrap().len(), 64);
                }
            }
        }
    }
}

#[test]
fn stage4_artifact_only_validation_writes_no_receipt() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (program, script, receipt_name) in [
        (
            "python3",
            "tools/vnext_contracts/stage4/execution/validate.py",
            "semantic-validation-receipt.v1.json",
        ),
        (
            "ruby",
            "tools/vnext_contracts/stage4/execution/verify.rb",
            "ruby-verification-receipt.v1.json",
        ),
    ] {
        let temporary = TemporaryRoot::new(receipt_name);
        let root = temporary.0.join("execution");
        copy_tree(&repo.join("contracts/vnext/stage4/execution"), &root);
        fs::remove_file(root.join(receipt_name)).expect("remove copied receipt");
        let output = run(
            repo,
            program,
            &[
                script,
                "--root",
                root.to_str().expect("UTF-8 root"),
                "--artifact-only",
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!root.join(receipt_name).exists());
    }
}

#[test]
fn stage4_build_check_rejects_a_skipped_certification_receipt() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("skipped-receipt");
    let root = temporary.0.join("execution");
    copy_tree(&repo.join("contracts/vnext/stage4/execution"), &root);
    let receipt_path = root.join("semantic-validation-receipt.v1.json");
    let mut receipt = read_json(&receipt_path);
    receipt["validation_mode"] = json!("artifact_only");
    receipt["predecessor_chain"]["mode"] = json!("skipped");
    write_json(&receipt_path, &receipt);
    let output = run(
        repo,
        "python3",
        &[
            "tools/vnext_contracts/stage4/execution/build.py",
            "--check",
            "--root",
            root.to_str().expect("UTF-8 root"),
        ],
    );
    assert!(
        !output.status.success(),
        "Stage 4 accepted a skipped receipt"
    );
}

#[test]
fn stage4_source_closure_is_live_exact_and_excludes_stage5() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest =
        read_json(&repo.join("contracts/vnext/stage4/execution/execution-effects.v1.json"));
    let paths = source_paths(&manifest);
    for required in [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "src/lib.rs",
        "src/domain/mod.rs",
        "src/domain/mod.rs",
        "src/domain/authority/action_basis.rs",
        "src/domain/authority/continuity/trusted_time.rs",
        "src/domain/evidence/submission_claim.rs",
        "src/domain/evidence/claim.rs",
        "src/domain/execution/mod.rs",
        "src/domain/persistence/mod.rs",
        "src/domain/step/lifecycle.rs",
        "src/domain/step/submission.rs",
        "src/foundation/core/deterministic_cbor.rs",
    ] {
        assert!(
            paths.contains(&required),
            "missing Stage 4 source {required}"
        );
    }
    assert!(
        paths
            .iter()
            .any(|path| path.starts_with("src/domain/execution/"))
    );
    assert!(
        paths
            .iter()
            .any(|path| path.starts_with("src/domain/persistence/"))
    );
    assert!(paths.contains(&"src/domain/evidence/claim.rs"));
    assert!(paths.contains(&"src/domain/evidence/submission_claim.rs"));
    assert!(!paths.iter().any(|path| {
        path.starts_with("src/domain/evidence/")
            && !matches!(
                *path,
                "src/domain/evidence/mod.rs"
                    | "src/domain/evidence/claim.rs"
                    | "src/domain/evidence/submission_claim.rs"
            )
    }));
    assert!(
        !paths
            .iter()
            .any(|path| path.starts_with("src/domain/gate/"))
    );

    let actual_execution = fs::read_dir(repo.join("src/domain/execution"))
        .expect("read execution root")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("rs"))
        .map(|entry| {
            format!(
                "src/domain/execution/{}",
                entry.file_name().to_string_lossy()
            )
        })
        .collect::<Vec<_>>();
    for path in actual_execution {
        assert!(
            paths.contains(&path.as_str()),
            "omitted live execution source {path}"
        );
    }
    let toolchain = &manifest["canonical_value"][10];
    assert_eq!(toolchain[0], "proof_toolchain_environment_and_target_v2");
    assert!(
        toolchain[4]
            .as_array()
            .expect("rustc identity")
            .iter()
            .any(|line| line.as_str().is_some_and(|line| line.starts_with("host: ")))
    );
    assert!(
        toolchain[6]
            .as_array()
            .expect("target cfg")
            .iter()
            .any(|line| line
                .as_str()
                .is_some_and(|line| line.starts_with("target_feature=")))
    );
    let path = toolchain[2]
        .as_array()
        .expect("bound proof environment")
        .iter()
        .find(|row| row[0] == "PATH")
        .and_then(|row| row[1].as_str())
        .expect("bound PATH");
    assert!(path.contains("<codex-transient-arg0>"));
    assert!(!path.contains("/.codex/tmp/arg0/codex-arg0"));
}

#[test]
fn stage4_transient_codex_arg0_paths_normalize_to_one_identity() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let current_path = std::env::var("PATH").expect("PATH");
    let transient = |suffix: &str| {
        let mut replaced = 0;
        let components = current_path
            .split(':')
            .map(|component| {
                if component.contains("/.codex/tmp/arg0/codex-arg0") {
                    replaced += 1;
                    let prefix = component
                        .split_once("/.codex/tmp/arg0/codex-arg0")
                        .expect("transient Codex PATH shape")
                        .0;
                    format!("{prefix}/.codex/tmp/arg0/codex-arg0{suffix}")
                } else {
                    component.to_owned()
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(replaced, 1, "expected one transient Codex PATH component");
        components.join(":")
    };
    let mut identities = Vec::new();
    for suffix in ["First", "Second"] {
        let output = Command::new("python3")
            .args([
                "tools/vnext_contracts/stage4/execution/validate.py",
                "--root",
                "contracts/vnext/stage4/execution",
                "--artifact-only",
            ])
            .env("PATH", transient(suffix))
            .current_dir(repo)
            .output()
            .expect("run independent Stage 4 reconstruction");
        assert!(
            output.status.success(),
            "transient PATH reconstruction failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        identities.push(String::from_utf8(output.stdout).unwrap());
    }
    assert_eq!(identities[0].trim(), identities[1].trim());
}

#[test]
fn stage4_artifact_binds_exact_frozen_catalog_dispatch_and_withdrawal_closure() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest =
        read_json(&repo.join("contracts/vnext/stage4/execution/execution-effects.v1.json"));
    let value = &manifest["canonical_value"];
    assert_eq!(value[4][1], json!([145, 11, 23, 139, 3, 156]));
    assert_eq!(value[6][1].as_array().expect("dispatch outcomes").len(), 4);
    assert_eq!(value[7][1], 60);
    assert_eq!(value[7][2], 21);
    assert_eq!(
        value[7][5],
        "withdrawn locally; no provider cancellation performed"
    );
}

#[test]
fn runtime_withdrawal_catalog_matches_all_sixty_frozen_rows_and_twenty_one_denials() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let frozen =
        read_json(&repo.join("contracts/vnext/stage0/effect-home/effect-withdrawal-v1.json"));
    let runtime_rows = withdrawal_catalog_cells_v1()
        .into_iter()
        .map(|cell| {
            json!([
                cell.classification_literal(),
                cell.compatibility_identity(),
                cell.origin_tag(),
                cell.route_tag(),
                cell.branch_tag(),
                cell.role_literal(),
                cell.home_literal(),
                cell.catalog_descriptor_id(),
                cell.semantic_binding(),
            ])
        })
        .collect::<Vec<_>>();
    assert_eq!(Value::Array(runtime_rows), frozen["canonical_value"][2]);

    let denied = WithdrawalDeniedProductV1::ALL
        .map(WithdrawalDeniedProductV1::literal)
        .map(Value::from);
    assert_eq!(Value::Array(denied.to_vec()), frozen["canonical_value"][4]);
}

#[test]
fn independent_execution_artifact_rejects_semantic_and_shape_mutants() {
    let cases: Vec<ManifestMutant> = vec![
        (
            "skipped-chain",
            Box::new(|value| value["canonical_value"][3][0] = json!("skipped")),
        ),
        (
            "fourth-owner",
            Box::new(|value| {
                value["canonical_value"][5][0][2]
                    .as_array_mut()
                    .expect("owners")
                    .push(json!(["RetryAttemptV1", "forbidden", "forbidden"]))
            }),
        ),
        (
            "lease-donation",
            Box::new(|value| value["canonical_value"][5][1][1] = json!("all_attempts")),
        ),
        (
            "run-state-smuggling",
            Box::new(|value| {
                value["canonical_value"][5][2][2][0][1]
                    .as_array_mut()
                    .expect("Run successors")
                    .push(json!("running"))
            }),
        ),
        (
            "stale-step-mutation",
            Box::new(|value| {
                canonical_model_row_mut(value, "ReconciliationAttemptV1")[7] =
                    json!("may_mutate_origin_step")
            }),
        ),
        (
            "retry-engine",
            Box::new(|value| {
                canonical_model_row_mut(value, "retry_policy")[1] = json!("retry_engine")
            }),
        ),
        (
            "fifth-dispatch-payload",
            Box::new(|value| {
                value["canonical_value"][6][1]
                    .as_array_mut()
                    .expect("dispatch payloads")
                    .push(json!([5, "retryable", 2]))
            }),
        ),
        (
            "withdrawal-count",
            Box::new(|value| value["canonical_value"][7][1] = json!(59)),
        ),
        (
            "catalog-count",
            Box::new(|value| value["canonical_value"][4][1][0] = json!(144)),
        ),
        (
            "missing-persistence-source",
            Box::new(|value| {
                value["canonical_value"][9]
                    .as_array_mut()
                    .expect("source rows")
                    .retain(|row| {
                        !row[0]
                            .as_str()
                            .expect("source path")
                            .starts_with("src/domain/persistence/")
                    })
            }),
        ),
        (
            "missing-step-submission-owner",
            Box::new(|value| {
                value["canonical_value"][5]
                    .as_array_mut()
                    .expect("execution model")
                    .retain(|row| row[0] != "StepSubmissionV1")
            }),
        ),
        (
            "embedded-claim-records",
            Box::new(|value| {
                value["canonical_value"][5][12][1] =
                    json!("step_owned_submission_embeds_claim_records")
            }),
        ),
        (
            "changed-claim-set-digest",
            Box::new(|value| {
                value["canonical_value"][5][12][1] =
                    json!("step_owned_submission_stores_changed_claim_set_digest")
            }),
        ),
        (
            "non-atomic-submission-participants",
            Box::new(|value| {
                value["canonical_value"][5][12][4] =
                    json!("persistence_commits_participants_in_separate_generations")
            }),
        ),
        (
            "unknown-field",
            Box::new(|value| value["unexpected"] = json!(true)),
        ),
    ];
    for (name, mutate) in cases {
        assert_rejected_by_both(name, mutate);
    }
}

#[test]
fn stage4_proof_rejects_missing_runtime_owner_types() {
    let (_temporary, workspace) = mutant_workspace("missing-owner-types");
    let count = replace_in_tree(
        &workspace.join("src/domain/execution"),
        "ExecutionAttemptV1",
        "ExecutionAttemptMutantV1",
    );
    assert!(
        count > 0,
        "Stage 4 source contains no ExecutionAttemptV1 to mutate"
    );
    assert_workspace_rejected_by_both(&workspace, "missing-owner-types");
    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    assert!(
        !output.status.success(),
        "Stage 4 regenerated an artifact without the closed runtime owner\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("runtime semantics"),
        "Stage 4 owner mutant failed outside the semantic gate\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stage4_proof_rejects_missing_atomic_store_publication() {
    let (_temporary, workspace) = mutant_workspace("missing-store-publication");
    let count = replace_in_tree(
        &workspace.join("src/domain/persistence"),
        "publish",
        "publ1sh",
    );
    assert!(
        count > 0,
        "Stage 4 persistence source contains no publication seam"
    );
    assert_workspace_rejected_by_both(&workspace, "missing-store-publication");
    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    assert!(
        !output.status.success(),
        "Stage 4 regenerated an artifact without Store publication\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("publication semantics"),
        "Stage 4 Store mutant failed outside the semantic gate\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stage4_proof_rejects_execution_store_disconnection() {
    let (_temporary, workspace) = mutant_workspace("store-disconnection");
    let count = replace_in_tree(
        &workspace.join("src/domain/execution"),
        "persistence",
        "detached_store",
    );
    assert!(
        count > 0,
        "Stage 4 Execution source contains no persistence binding"
    );
    assert_workspace_rejected_by_both(&workspace, "store-disconnection");
    let output = regenerate(&workspace, &workspace.join("proof/execution"));
    assert!(
        !output.status.success(),
        "Stage 4 regenerated an artifact with a disconnected Store\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("does not bind the canonical persistence owner"),
        "Stage 4 Store-disconnection mutant failed outside the semantic gate\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stage4_proof_rejects_stored_test_census_drift() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("stored-test-census-drift");
    let expected = temporary.0.join("expected");
    let actual = temporary.0.join("actual");
    copy_tree(&repo.join("contracts/vnext/stage4/execution"), &expected);
    copy_tree(&expected, &actual);
    let receipt_path = actual.join("behavioral-proof-receipt.v1.json");
    let mut receipt = read_json(&receipt_path);
    let row = receipt["command_receipts"][0]
        .as_object_mut()
        .expect("first behavior receipt");
    row.get_mut("test_names")
        .and_then(Value::as_array_mut)
        .and_then(|names| names.first_mut())
        .map(|name| *name = json!("mutant::invented_passing_test"))
        .expect("stored exact test name");
    let outcome = json!({
        "command": row["command"].clone(),
        "ignored": row["ignored"].clone(),
        "passed": row["passed"].clone(),
        "test_binary": row["test_binary"].clone(),
        "test_names": row["test_names"].clone(),
    });
    row.insert(
        "normalized_output_sha256".to_owned(),
        json!(sha256_hex(
            &serde_json::to_vec(&outcome).expect("canonical receipt outcome")
        )),
    );
    write_json(&receipt_path, &receipt);
    assert_proof_tree_drift_rejected(repo, &expected, &actual, "a rewritten exact test census");
}

#[test]
fn stage4_proof_rejects_substituted_test_binary_receipt() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("substituted-test-binary");
    let expected = temporary.0.join("expected");
    let actual = temporary.0.join("actual");
    copy_tree(&repo.join("contracts/vnext/stage4/execution"), &expected);
    copy_tree(&expected, &actual);
    let receipt_path = actual.join("behavioral-proof-receipt.v1.json");
    let mut receipt = read_json(&receipt_path);
    let row = receipt["command_receipts"][0]
        .as_object_mut()
        .expect("first behavior receipt");
    let substituted =
        fs::read(actual.join("execution-effects.v1.json")).expect("read substituted binary bytes");
    row.insert(
        "test_binary".to_owned(),
        json!({
            "byte_length": substituted.len(),
            "path": "contracts/vnext/stage4/execution/execution-effects.v1.json",
            "sha256": sha256_hex(&substituted),
        }),
    );
    let outcome = json!({
        "command": row["command"].clone(),
        "ignored": row["ignored"].clone(),
        "passed": row["passed"].clone(),
        "test_binary": row["test_binary"].clone(),
        "test_names": row["test_names"].clone(),
    });
    row.insert(
        "normalized_output_sha256".to_owned(),
        json!(sha256_hex(
            &serde_json::to_vec(&outcome).expect("canonical receipt outcome")
        )),
    );
    write_json(&receipt_path, &receipt);
    assert_proof_tree_drift_rejected(
        repo,
        &expected,
        &actual,
        "a substituted compiled test binary receipt",
    );
}

#[test]
fn stage4_proof_rejects_independent_reexecution_divergence() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = TemporaryRoot::new("independent-reexecution-divergence");
    let expected = temporary.0.join("expected");
    let actual = temporary.0.join("actual");
    copy_tree(&repo.join("contracts/vnext/stage4/execution"), &expected);
    copy_tree(&expected, &actual);
    let receipt_path = actual.join("semantic-validation-receipt.v1.json");
    let mut receipt = read_json(&receipt_path);
    let row = receipt["behavioral_reexecution"]["command_receipts"][0]
        .as_object_mut()
        .expect("first independent behavior receipt");
    row.get_mut("test_names")
        .and_then(Value::as_array_mut)
        .and_then(|names| names.first_mut())
        .map(|name| *name = json!("mutant::independent_only_test"))
        .expect("independent exact test name");
    let outcome = json!({
        "command": row["command"].clone(),
        "ignored": row["ignored"].clone(),
        "passed": row["passed"].clone(),
        "test_binary": row["test_binary"].clone(),
        "test_names": row["test_names"].clone(),
    });
    row.insert(
        "normalized_output_sha256".to_owned(),
        json!(sha256_hex(
            &serde_json::to_vec(&outcome).expect("canonical receipt outcome")
        )),
    );
    write_json(&receipt_path, &receipt);
    assert_proof_tree_drift_rejected(
        repo,
        &expected,
        &actual,
        "a divergent independent reexecution receipt",
    );
}

#[test]
fn stage4_benign_source_byte_change_passes_after_regeneration() {
    let _gate = COMPILED_MUTANT_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_temporary, workspace) = compiled_mutant_workspace("benign-source-byte-change");
    let parent_certification_identity = stage4_certification_identity(&workspace);
    let source_path = workspace.join("src/domain/execution/mod.rs");
    let mut source = fs::read_to_string(&source_path).expect("read Execution facade");
    source.push_str("\n// Stage 4 benign source-closure regeneration probe.\n");
    fs::write(&source_path, source).expect("write benign Execution source mutation");

    for (program, script) in validators() {
        let output = run(&workspace, program, &[script, "--source-only"]);
        assert!(
            output.status.success(),
            "{program} rejected a benign source-byte mutation before regeneration\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let regenerated = workspace.join("proof/execution");
    let output = regenerate(&workspace, &regenerated);
    assert!(
        output.status.success(),
        "Stage 4 could not regenerate after a benign source-byte mutation\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for (program, script) in validators() {
        let output = run(
            &workspace,
            program,
            &[
                script,
                "--root",
                regenerated.to_str().expect("UTF-8 regenerated root"),
                "--artifact-only",
                "--skip-mutants",
                "--parent-certification-identity",
                &parent_certification_identity,
            ],
        );
        assert!(
            output.status.success(),
            "{program} rejected the regenerated benign source artifact\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
