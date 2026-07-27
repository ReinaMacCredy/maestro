#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use crate::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::domain::vnext::migration::runtime::{
        ByteTotalInventoryV1, CancellationClassificationV1, ClassificationSetV1, ClientAdmissionV1,
        ClientRefusalReasonV1, ConsumerAccessV1, ConsumerCensusEntryV1, ConsumerClosureV1,
        ConsumerGateStageV1, ConsumerGenerationV1, ConsumerRecordV1, ConsumerSubjectV1,
        CutoverAcceptanceV1, DeclaredRootV1, DeterministicIdentityMapV1, EffectCrossingV1,
        IdentityMapEntryV1, IdentityMappingBasisV1, InventoryDomainV1, InventoryNodeKindV1,
        InventoryPayloadV1, InventoryRowV1, MigrationDigestV1, MigrationDispositionV1,
        MigrationProtocolClosureV1, NormalizedLocatorV1, ProtectedV1RollbackOutcomeV1,
        PrunePrerequisitesV1, QuarantineEntryV1, RollbackAssessmentV1, RollbackDispositionV1,
        RollbackRestoreErrorV1, SealedQuarantineManifestV1, SourceClassificationV1,
        restore_protected_exact_v1,
    };
    use crate::domain::vnext::persistence::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreObjectV1, StoreRoleV1,
        StoreStateV1, StoreV1,
    };
    use crate::foundation::core::deterministic_cbor::CborValue;
    use crate::operations::vnext::migration::import_inactive_store;

    const INSTANCE_FIXTURE: &[u8] =
        include_bytes!("../../../../../tests/fixtures/vnext/stage11/migration_instances.v1.jsonl");
    const COHORT_FIXTURE: &[u8] = include_bytes!(
        "../../../../../tools/vnext_contracts/final_chain/fixtures/migration-cohorts.v1.json"
    );
    const COHORT_OBSERVATION_SCHEMA: &str = "maestro.external.vnext-final-cohort-observation.v1";
    const ROUTE_OBSERVATION_SCHEMA: &str =
        "maestro.external.vnext-final-cohort-route-observation.v1";
    const EXPECTED_INSTANCE_DIGEST: &str =
        "b4f869a16328f0ccb640a5c3cc9a7c6bd9b1d295c614eb82fddfb680496498d5";
    const EXPECTED_ROW_COUNTS: [(&str, usize); 3] =
        [("c325", 325), ("e204", 204), ("skill_ledger", 35)];
    static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug)]
    struct TemporaryRoot(PathBuf);

    impl TemporaryRoot {
        fn new() -> Self {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary =
                fs::canonicalize(env::temp_dir()).expect("canonical temporary directory");
            let path = temporary.join(format!(
                "maestro-stage11-cohort-{}-{sequence}",
                std::process::id()
            ));
            if path.exists() {
                fs::remove_dir_all(&path).expect("remove stale Stage11 cohort root");
            }
            fs::create_dir(&path).expect("create Stage11 cohort root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove Stage11 cohort root");
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct FrozenRows {
        digest: MigrationDigestV1,
        counts: BTreeMap<String, usize>,
        row_count: usize,
    }

    impl FrozenRows {
        fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
            let digest = MigrationDigestV1::digest_bytes(bytes).map_err(|_| "fixture digest")?;
            if digest.render_hex() != EXPECTED_INSTANCE_DIGEST {
                return Err("fixture identity differs");
            }
            let mut lines = bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty());
            let header: Value =
                serde_json::from_slice(lines.next().ok_or("fixture header missing")?)
                    .map_err(|_| "fixture header invalid")?;
            if header["schema"] != "maestro.vnext.stage11.migration-instances.v1" {
                return Err("fixture schema differs");
            }
            let declared = header["row_counts"]
                .as_object()
                .ok_or("fixture row counts missing")?;
            let mut counts = BTreeMap::<String, usize>::new();
            let mut ordinals = BTreeMap::<String, usize>::new();
            for raw in lines {
                let row: Value = serde_json::from_slice(raw).map_err(|_| "fixture row invalid")?;
                let family = row["family"]
                    .as_str()
                    .ok_or("fixture family missing")?
                    .to_string();
                let next = counts.get(&family).copied().unwrap_or(0) + 1;
                if row["ordinal"].as_u64() != Some(next as u64) || row.get("row").is_none() {
                    return Err("fixture row coverage differs");
                }
                counts.insert(family.clone(), next);
                ordinals.insert(family, next);
            }
            let expected = EXPECTED_ROW_COUNTS
                .into_iter()
                .map(|(family, count)| (family.to_string(), count))
                .collect::<BTreeMap<_, _>>();
            if counts != expected
                || declared.len() != expected.len()
                || expected.iter().any(|(family, count)| {
                    declared.get(family).and_then(Value::as_u64) != Some(*count as u64)
                })
                || ordinals != expected
            {
                return Err("fixture family closure differs");
            }
            let row_count = counts.values().sum();
            Ok(Self {
                digest,
                counts,
                row_count,
            })
        }

        fn typed_binding(&self) -> String {
            format!(
                "rows=c325:{},e204:{},skill_ledger:{};total={};fixture=sha256:{}",
                self.counts["c325"],
                self.counts["e204"],
                self.counts["skill_ledger"],
                self.row_count,
                self.digest
            )
        }
    }

    fn digest(label: &[u8], fixture: &[u8]) -> MigrationDigestV1 {
        let mut bytes = Vec::with_capacity(label.len() + fixture.len() + 1);
        bytes.extend_from_slice(label);
        bytes.push(0);
        bytes.extend_from_slice(fixture);
        MigrationDigestV1::digest_bytes(&bytes).expect("fixture-derived nonzero digest")
    }

    fn locator(value: &str) -> NormalizedLocatorV1 {
        NormalizedLocatorV1::new(value.as_bytes().to_vec()).expect("normalized cohort locator")
    }

    fn protocol(fixture: &[u8]) -> MigrationProtocolClosureV1 {
        MigrationProtocolClosureV1::new(
            digest(b"association-schema", fixture),
            digest(b"active-head-schema", fixture),
            digest(b"pre-store-seal-schema", fixture),
            digest(b"finality-edge-manifest", fixture),
            digest(b"schema-read-write-set", fixture),
            digest(b"writer-protocol-epoch", fixture),
            digest(b"migration-epoch", fixture),
            Some(digest(b"release", fixture)),
        )
        .expect("fixture-derived protocol closure")
    }

    fn consumer_closure(
        fixture: &[u8],
        record: ConsumerRecordV1,
        expected: MigrationProtocolClosureV1,
    ) -> ConsumerClosureV1 {
        ConsumerClosureV1::evaluate_owner_snapshot_parts(
            ConsumerGateStageV1::BeforeSemanticCurrentness,
            expected,
            1,
            [
                digest(b"consumer-source-manifest", fixture),
                digest(b"consumer-owner-snapshot", fixture),
                digest(b"consumer-closure-attestation", fixture),
            ],
            vec![ConsumerCensusEntryV1::observed(
                digest(b"consumer-observation", fixture),
                record,
            )],
            PrunePrerequisitesV1::blocked(),
        )
        .expect("typed consumer closure")
    }

    fn old_reader_route(rows: &FrozenRows, fixture: &[u8]) -> String {
        let expected = protocol(fixture);
        let sealed = ConsumerRecordV1::new(
            locator("/stage11/cohort/old-reader"),
            ConsumerSubjectV1::LegacySource,
            ConsumerGenerationV1::LegacyV1,
            ConsumerAccessV1::SealedMigrationReader,
            false,
            false,
            None,
        )
        .expect("sealed old reader");
        let closure = consumer_closure(fixture, sealed, expected.clone());
        assert!(closure.gate_passed());
        assert_eq!(
            closure.admissions(),
            &[(
                closure.consumers()[0].id(),
                ClientAdmissionV1::OpaqueSealedOnly
            )]
        );

        let active_old = ConsumerRecordV1::new(
            locator("/stage11/cohort/old-reader-active-mutant"),
            ConsumerSubjectV1::LegacySource,
            ConsumerGenerationV1::LegacyV1,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            None,
        )
        .expect("active old-reader mutant");
        let refused = consumer_closure(fixture, active_old, expected);
        assert!(!refused.gate_passed());
        assert!(matches!(
            refused.admissions(),
            [(
                _,
                ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::OldProtocol)
            )]
        ));
        format!("opaque_sealed_only;{}", rows.typed_binding())
    }

    fn new_reader_route(rows: &FrozenRows, fixture: &[u8]) -> (String, ConsumerClosureV1) {
        let expected = protocol(fixture);
        let current = ConsumerRecordV1::new(
            locator("/stage11/cohort/new-reader"),
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            Some(expected.clone()),
        )
        .expect("exact current reader");
        let closure = consumer_closure(fixture, current, expected.clone());
        assert!(closure.gate_passed());
        assert_eq!(
            closure.admissions(),
            &[(closure.consumers()[0].id(), ClientAdmissionV1::ExactCurrent)]
        );

        let mut mutant = fixture.to_vec();
        let last = mutant.last_mut().expect("nonempty fixture");
        *last ^= 1;
        let mixed = ConsumerRecordV1::new(
            locator("/stage11/cohort/new-reader-protocol-mutant"),
            ConsumerSubjectV1::CurrentTarget,
            ConsumerGenerationV1::CurrentVNext,
            ConsumerAccessV1::ActiveRuntime,
            true,
            true,
            Some(protocol(&mutant)),
        )
        .expect("mixed protocol mutant");
        let refused = consumer_closure(fixture, mixed, expected);
        assert!(!refused.gate_passed());
        assert!(matches!(
            refused.admissions(),
            [(
                _,
                ClientAdmissionV1::RefusedBeforeCurrentness(ClientRefusalReasonV1::ReleaseMismatch)
            )]
        ));
        (format!("exact_current;{}", rows.typed_binding()), closure)
    }

    struct WriterOutcome {
        typed_result: String,
        request_id: MigrationDigestV1,
    }

    fn writer_commit_route(
        rows: &FrozenRows,
        fixture: &[u8],
        consumers: &ConsumerClosureV1,
        temporary: &TemporaryRoot,
    ) -> WriterOutcome {
        let root = DeclaredRootV1::new(
            locator("/stage11/frozen"),
            locator("/stage11/frozen"),
            InventoryDomainV1::Repository,
            false,
        )
        .expect("fixture root");
        let row = InventoryRowV1::new(
            root.id(),
            locator("/stage11/frozen/migration_instances.v1.jsonl"),
            locator("/stage11/frozen/migration_instances.v1.jsonl"),
            InventoryDomainV1::Repository,
            InventoryNodeKindV1::RegularFile,
            InventoryPayloadV1::from_bytes(fixture).expect("fixture payload"),
            digest(b"fixture-metadata", fixture),
        )
        .expect("fixture inventory row");
        let inventory =
            ByteTotalInventoryV1::new(vec![root], vec![row.clone()]).expect("fixture inventory");

        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage11-cohort-writer")
            .expect("writer domain");
        let compatibility =
            StoreCompatibilityV1::stage0_successor().expect("frozen Store compatibility");
        let object = StoreObjectV1::new(
            SchemaIdV1::parse(&format!("sha256:{}", digest(b"schema", fixture)))
                .expect("fixture schema identity"),
            CborValue::Bytes(fixture.to_vec()),
            vec![],
        )
        .expect("fixture Store object");
        let mut source = StoreV1::create(temporary.path().join("writer-source"), domain.clone())
            .expect("source Store");
        source.put_object(&object).expect("persist fixture object");
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&format!("sha256:{}", digest(b"contract-root", fixture)))
                .expect("fixture Contract Root"),
            compatibility,
            vec![object.id()],
        )
        .expect("source Generation");
        source
            .publish_generation(&generation, None)
            .expect("publish fixture Generation");
        let backup = source.seal_export().expect("sealed fixture export");

        let target_id = MigrationDigestV1::from_digest(object.id().into_bytes())
            .expect("fixture target identity");
        let classifications = ClassificationSetV1::new(
            &inventory,
            vec![
                SourceClassificationV1::new(
                    row.source_id(),
                    MigrationDispositionV1::OpaquePreserved,
                    digest(b"writer-classification", fixture),
                    Some(target_id),
                    None,
                    false,
                    CancellationClassificationV1::NotCancellationLike,
                )
                .expect("writer classification"),
            ],
        )
        .expect("writer classification set");
        let target_map = DeterministicIdentityMapV1::new(
            &classifications,
            vec![
                IdentityMapEntryV1::new(
                    row.source_id(),
                    target_id,
                    IdentityMappingBasisV1::HistoricalOpaque {
                        preservation_proof_id: digest(b"preservation-proof", fixture),
                    },
                )
                .expect("writer identity map row"),
            ],
        )
        .expect("writer identity map");
        let quarantine = SealedQuarantineManifestV1::new(
            &inventory,
            &classifications,
            locator("/stage11/quarantine"),
            vec![],
        )
        .expect("empty writer quarantine");
        let request = crate::domain::vnext::migration::runtime::InactiveStoreImportRequestV1::new(
            &inventory,
            &classifications,
            &target_map,
            &quarantine,
            consumers,
            backup.canonical_bytes(),
            object.id(),
        )
        .expect("inactive writer request");

        let mut corrupted = backup.canonical_bytes().to_vec();
        corrupted[0] ^= 1;
        let mut refused = StoreV1::create(temporary.path().join("writer-refused"), domain.clone())
            .expect("refused Store");
        assert!(import_inactive_store(&mut refused, &request, &corrupted).is_err());
        assert_eq!(
            refused.state().expect("refused Store state").0,
            StoreStateV1::Inactive
        );
        assert!(refused.active_head().expect("refused Store head").is_none());

        let mut destination = StoreV1::create(temporary.path().join("writer-destination"), domain)
            .expect("destination Store");
        let receipt = import_inactive_store(&mut destination, &request, backup.canonical_bytes())
            .expect("inactive writer commit");
        assert_eq!(
            destination.state().expect("destination state").0,
            StoreStateV1::Inactive
        );
        assert!(
            destination
                .active_head()
                .expect("destination head")
                .is_none()
        );
        let restored = destination
            .read_object(object.id())
            .expect("read committed fixture object");
        assert_eq!(restored, object);
        assert_eq!(restored.value(), &CborValue::Bytes(fixture.to_vec()));
        assert!(!receipt.activated());
        assert!(!receipt.claims_currentness());
        assert_eq!(
            receipt.candidate_root_id().as_bytes(),
            object.id().as_bytes()
        );
        let receipt_digest =
            MigrationDigestV1::digest_bytes(receipt.canonical_bytes()).expect("receipt digest");
        WriterOutcome {
            typed_result: format!(
                "inactive_store_commit;{};receipt=sha256:{receipt_digest}",
                rows.typed_binding()
            ),
            request_id: request.id(),
        }
    }

    fn rollback_route(rows: &FrozenRows, fixture: &[u8], attempt: MigrationDigestV1) -> String {
        let root = DeclaredRootV1::new(
            locator("/stage11/rollback-source"),
            locator("/stage11/rollback-source"),
            InventoryDomainV1::Repository,
            false,
        )
        .expect("rollback root");
        let row = InventoryRowV1::new(
            root.id(),
            locator("/stage11/rollback-source/migration_instances.v1.jsonl"),
            locator("/stage11/rollback-source/migration_instances.v1.jsonl"),
            InventoryDomainV1::Repository,
            InventoryNodeKindV1::RegularFile,
            InventoryPayloadV1::from_bytes(fixture).expect("rollback payload"),
            digest(b"rollback-metadata", fixture),
        )
        .expect("rollback inventory row");
        let inventory =
            ByteTotalInventoryV1::new(vec![root], vec![row.clone()]).expect("rollback inventory");
        let entry = QuarantineEntryV1::new(
            &row,
            fixture.to_vec(),
            digest(b"rollback-reason", fixture),
            digest(b"rollback-recovery", fixture),
        )
        .expect("sealed rollback entry");
        let classifications = ClassificationSetV1::new(
            &inventory,
            vec![
                SourceClassificationV1::new(
                    row.source_id(),
                    MigrationDispositionV1::Quarantined,
                    entry.reason_id(),
                    None,
                    Some(entry.id()),
                    false,
                    CancellationClassificationV1::NotCancellationLike,
                )
                .expect("rollback classification"),
            ],
        )
        .expect("rollback classification set");
        SealedQuarantineManifestV1::new(
            &inventory,
            &classifications,
            locator("/stage11/rollback-quarantine"),
            vec![entry.clone()],
        )
        .expect("sealed rollback manifest");

        let eligible = RollbackAssessmentV1::assess_cutover_observation(
            attempt,
            Some(attempt),
            CutoverAcceptanceV1::PreAccept,
            EffectCrossingV1::ProvenNotCrossed,
        )
        .expect("eligible rollback assessment");
        let restored = restore_protected_exact_v1(&eligible, &entry, fixture)
            .expect("protected exact-v1 restore");
        let ProtectedV1RollbackOutcomeV1::Restored {
            source_id,
            source_sha256,
            bytes,
        } = restored
        else {
            panic!("eligible rollback was refused");
        };
        assert_eq!(source_id, row.source_id());
        assert_eq!(source_sha256, rows.digest);
        assert_eq!(bytes, fixture);

        let mut corrupted = fixture.to_vec();
        corrupted[0] ^= 1;
        assert_eq!(
            restore_protected_exact_v1(&eligible, &entry, &corrupted),
            Err(RollbackRestoreErrorV1::ProtectedBytesMismatch)
        );

        for (observed_attempt_id, acceptance, effect_crossing, expected) in [
            (
                Some(digest(b"stale-attempt", fixture)),
                CutoverAcceptanceV1::PreAccept,
                EffectCrossingV1::ProvenNotCrossed,
                RollbackDispositionV1::RefusedStaleHost,
            ),
            (
                Some(attempt),
                CutoverAcceptanceV1::Accepted,
                EffectCrossingV1::ConfirmedCrossed,
                RollbackDispositionV1::VNextFreshGenerationRecoveryOnly,
            ),
        ] {
            let refused = RollbackAssessmentV1::assess_cutover_observation(
                attempt,
                observed_attempt_id,
                acceptance,
                effect_crossing,
            )
            .expect("typed rollback refusal assessment");
            assert_eq!(
                restore_protected_exact_v1(&refused, &entry, fixture)
                    .expect("typed rollback refusal"),
                ProtectedV1RollbackOutcomeV1::Refused {
                    disposition: expected
                }
            );
        }
        format!(
            "restored_exact_v1_and_refused_stale_or_crossed;{}",
            rows.typed_binding()
        )
    }

    #[derive(Debug)]
    struct RouteOutcomes {
        old_reader: String,
        new_reader: String,
        writer: String,
        rollback: String,
    }

    fn execute_real_routes() -> RouteOutcomes {
        let rows = FrozenRows::parse(INSTANCE_FIXTURE).expect("exact frozen migration fixture");
        assert_eq!(rows.row_count, 564);
        let mut mutant = INSTANCE_FIXTURE.to_vec();
        mutant[0] ^= 1;
        assert_eq!(
            FrozenRows::parse(&mutant),
            Err("fixture identity differs"),
            "fixture byte substitution must fail before any route executes"
        );

        let old_reader = old_reader_route(&rows, INSTANCE_FIXTURE);
        let (new_reader, current_consumers) = new_reader_route(&rows, INSTANCE_FIXTURE);
        let temporary = TemporaryRoot::new();
        let writer = writer_commit_route(&rows, INSTANCE_FIXTURE, &current_consumers, &temporary);
        let rollback = rollback_route(&rows, INSTANCE_FIXTURE, writer.request_id);
        RouteOutcomes {
            old_reader,
            new_reader,
            writer: writer.typed_result,
            rollback,
        }
    }

    fn prefixed_digest(bytes: &[u8]) -> String {
        format!(
            "sha256:{}",
            MigrationDigestV1::digest_bytes(bytes).expect("nonzero observation digest")
        )
    }

    fn binding(path: &str, bytes: &[u8]) -> Value {
        json!({
            "path": path,
            "byte_length": bytes.len(),
            "sha256": prefixed_digest(bytes),
        })
    }

    fn executable_binding() -> Value {
        let target = PathBuf::from(
            env::var_os("CARGO_TARGET_DIR").expect("receipt requires CARGO_TARGET_DIR"),
        );
        let target = fs::canonicalize(target).expect("canonical target root");
        let executable =
            fs::canonicalize(env::current_exe().expect("current test executable path"))
                .expect("canonical test executable");
        let relative = executable
            .strip_prefix(&target)
            .expect("test executable is rooted in CARGO_TARGET_DIR");
        let path = relative
            .to_str()
            .expect("UTF-8 test executable path")
            .replace('\\', "/");
        assert!(!path.is_empty() && !path.split('/').any(|part| part == ".."));
        let bytes = fs::read(&executable).expect("read invoked test executable");
        let mut value = binding(&path, &bytes);
        value
            .as_object_mut()
            .expect("binding object")
            .insert("root".to_string(), Value::String("target".to_string()));
        value
    }

    fn route_observation(proof_id: &str, route: &str, typed_result: &str) -> Value {
        json!({
            "schema_version": ROUTE_OBSERVATION_SCHEMA,
            "proof_id": proof_id,
            "route": route,
            "typed_result": typed_result,
            "status": "observed",
        })
    }

    fn validate_receipt_shape(receipt: &Value, proof_id: &str, cohort_identity: &str) {
        let object = receipt.as_object().expect("cohort receipt object");
        assert_eq!(object.len(), 5);
        assert_eq!(receipt["schema_version"], COHORT_OBSERVATION_SCHEMA);
        assert_eq!(receipt["proof_id"], proof_id);
        assert_eq!(receipt["cohort_identity"], cohort_identity);
        let executables = receipt["executables"]
            .as_object()
            .expect("executable bindings");
        assert_eq!(executables.len(), 3);
        for role in ["old_reader", "new_reader", "writer"] {
            let binding = executables[role]
                .as_object()
                .expect("rooted executable binding");
            assert_eq!(binding.len(), 4);
            assert_eq!(binding["root"], "target");
            assert!(
                binding["byte_length"]
                    .as_u64()
                    .is_some_and(|length| length > 0)
            );
            assert!(
                binding["sha256"]
                    .as_str()
                    .is_some_and(|value| value.len() == 71 && value.starts_with("sha256:"))
            );
        }
        let outcomes = receipt["outcomes"].as_object().expect("route outcomes");
        assert_eq!(outcomes.len(), 4);
        for route in ["old_reader", "new_reader", "writer", "rollback"] {
            let outcome = outcomes[route].as_object().expect("typed route outcome");
            assert_eq!(outcome.len(), 2);
            assert!(
                outcome["typed_result"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            );
            let observation = outcome["observation"]
                .as_object()
                .expect("route observation binding");
            assert_eq!(observation.len(), 3);
            assert!(
                observation["byte_length"]
                    .as_u64()
                    .is_some_and(|length| length > 0)
            );
        }
    }

    fn emit_receipt_after_assertions(outcomes: &RouteOutcomes) {
        let Some(receipt_path) = env::var_os("MAESTRO_FINAL_PROOF_RECEIPT").map(PathBuf::from)
        else {
            return;
        };
        let proof_id =
            env::var("MAESTRO_FINAL_PROOF_ID").expect("cohort receipt requires proof id");
        assert!(proof_id.starts_with("s11-"));
        let cohort_path = PathBuf::from(
            env::var_os("MAESTRO_MIGRATION_COHORT_PATH")
                .expect("cohort receipt requires migration cohort path"),
        );
        let cohort_bytes = fs::read(&cohort_path).expect("read invoked migration cohort");
        assert_eq!(
            cohort_bytes, COHORT_FIXTURE,
            "invoked cohort descriptor bytes differ from the frozen cohort"
        );
        let descriptor: Value =
            serde_json::from_slice(&cohort_bytes).expect("valid migration cohort descriptor");
        assert_eq!(
            descriptor["fixture"]["sha256"],
            format!("sha256:{EXPECTED_INSTANCE_DIGEST}")
        );
        assert_eq!(
            descriptor["required_outcomes"],
            json!([
                "old_reader_typed_compatibility_or_refusal",
                "new_reader_typed_acceptance",
                "writer_typed_commit",
                "rollback_typed_restore"
            ])
        );

        let output_root = receipt_path.parent().expect("receipt output root");
        fs::create_dir_all(output_root).expect("create receipt output root");
        let route_results = [
            ("old_reader", outcomes.old_reader.as_str()),
            ("new_reader", outcomes.new_reader.as_str()),
            ("writer", outcomes.writer.as_str()),
            ("rollback", outcomes.rollback.as_str()),
        ];
        let mut outcome_values = serde_json::Map::new();
        for (route, typed_result) in route_results {
            let file_name = format!("{proof_id}-{route}-route-observation.v1.json");
            let observation = route_observation(&proof_id, route, typed_result);
            let bytes = serde_json::to_vec(&observation).expect("serialize route observation");
            fs::write(output_root.join(&file_name), &bytes).expect("write route observation");
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &fs::read(output_root.join(&file_name)).expect("read route observation")
                )
                .expect("parse route observation"),
                observation
            );
            outcome_values.insert(
                route.to_string(),
                json!({
                    "typed_result": typed_result,
                    "observation": binding(&file_name, &bytes),
                }),
            );
        }

        let executable = executable_binding();
        let cohort_identity = prefixed_digest(&cohort_bytes);
        let receipt = json!({
            "schema_version": COHORT_OBSERVATION_SCHEMA,
            "proof_id": proof_id,
            "cohort_identity": cohort_identity,
            "executables": {
                "old_reader": executable.clone(),
                "new_reader": executable.clone(),
                "writer": executable,
            },
            "outcomes": Value::Object(outcome_values),
        });
        validate_receipt_shape(&receipt, &proof_id, &cohort_identity);
        let bytes = serde_json::to_vec(&receipt).expect("serialize cohort observation");
        fs::write(&receipt_path, &bytes).expect("write final cohort observation receipt");
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(&receipt_path).expect("read final cohort receipt")
            )
            .expect("parse final cohort receipt"),
            receipt
        );
    }

    #[test]
    fn frozen_cohort_migration_observes_real_reader_and_writer_routes() {
        let outcomes = execute_real_routes();
        assert!(outcomes.old_reader.starts_with("opaque_sealed_only;rows="));
        assert!(outcomes.new_reader.starts_with("exact_current;rows="));
        assert!(outcomes.writer.starts_with("inactive_store_commit;rows="));
        assert!(outcomes.old_reader.contains(EXPECTED_INSTANCE_DIGEST));
        assert!(outcomes.new_reader.contains(EXPECTED_INSTANCE_DIGEST));
        assert!(outcomes.writer.contains(EXPECTED_INSTANCE_DIGEST));
        emit_receipt_after_assertions(&outcomes);
    }

    #[test]
    fn frozen_cohort_rollback_observes_restore_and_refusal_routes() {
        let outcomes = execute_real_routes();
        assert!(
            outcomes
                .rollback
                .starts_with("restored_exact_v1_and_refused_stale_or_crossed;rows=")
        );
        assert!(outcomes.rollback.contains(EXPECTED_INSTANCE_DIGEST));
        emit_receipt_after_assertions(&outcomes);
    }
}
