//! CI bump guard for shipped, version-gated resources.
//!
//! A committed `(group, name, version, tree-hash)` table for every resource that
//! extracts under the shared version gate (skills, the hook recorder script, the
//! harness protocol) plus the embedded schema contract packs, whose recorded
//! version is the family's current schema stamp. The test recomputes a hash over
//! each resource's files
//! (every relative path and bytes, in canonical sorted order) and asserts it
//! matches the recorded one. Editing any shipped resource turns this red, forcing
//! you to *notice* the edit and re-record the table (and, when the change is
//! user-visible, bump its version per `AGENTS.md`). It enforces acknowledgement,
//! not a mechanical bump.

use std::fs;

use include_dir::{Dir, include_dir};
use maestro::domain::skills::catalog::skills;
use maestro::foundation::core::hash::sha256_hex;

/// The shipped schema contract packs (WS5 / D6.2-B), one directory per artifact
/// family. Included here directly, independent of the runtime catalog, so the
/// guard never couples to kernel internals.
static SCHEMAS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/schemas");

/// The shipped hook recorder script (its `# maestro:hook-version:` comment is
/// the version marker the recorder and installer gate on).
const RECORD_SH: &str = include_str!("../embedded/hooks/record.sh");

/// The shipped harness protocol (its frontmatter `version:` is the gate marker).
const HARNESS_MD: &str = include_str!("../embedded/harness/HARNESS.md");
const RECOVERY_MD: &str = include_str!("../embedded/harness/RECOVERY.md");

/// The shipped code playbook, served from the binary instead of extracted.
static PLAYBOOK_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/playbook");
/// The shipped DESIGN.md catalog, served from the binary instead of extracted.
static DESIGN_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/design");
/// `(group, name, shipped version, sha256 tree-hash of the resource files)`.
const RESOURCE_VERSION_GUARD: [(&str, &str, &str, &str); 22] = [
    (
        "skill",
        "ask-maestro",
        "1.0.7",
        "61250fa7f9140495242bb7de441199f8d00fe0915686d3b1b644788b67370132",
    ),
    (
        "skill",
        "maestro-research",
        "1.0.1",
        "f6672d507fa78f2a40fc568d43af880ff4789a130ee44dc8f094c635b81ce486",
    ),
    (
        "skill",
        "maestro-card",
        "1.37.26",
        "a105d8a0c85c706cf8e809f8b33be549b6c4367ef1efef799c58c084905bfabf",
    ),
    (
        "skill",
        "maestro-witness",
        "1.0.2",
        "f11027171e322fdafd9ad673f1cf3933121e0ac635e10546205367648e7222b7",
    ),
    (
        "skill",
        "maestro-setup",
        "1.11.8",
        "990f09fe51c77b3c6eb60479cd309de55621da0d133c99a3d1da22426f9ca312",
    ),
    (
        "skill",
        "maestro-design",
        "1.36.18",
        "f23ee1fdcdd44ca42c8b83bcf5759f82774c765b3ef25a7afaaf0dccc4278a45",
    ),
    (
        "skill",
        "maestro-audit",
        "1.13.9",
        "550e0760c9b7a9307cb3a009fffba9384b96aa06d76f5f8d516e31e0429c2608",
    ),
    (
        "hook",
        "record.sh",
        "1.0.2",
        "c1a75218747b8f58ffcd216aa8177d68fffd83376ff82dcf2eb32e40ea2d2fe7",
    ),
    (
        "harness",
        "HARNESS.md",
        "1.29.27",
        "ae0da06ebe23a856430218c936d944ce5afbc22a0ea04457381e6ba1d1066019",
    ),
    (
        "playbook",
        "PLAYBOOK.md",
        "binary-served",
        "39662b7afe1a4b9c45c859aecea6de5206923b6284192126b15cc280aa9836e8",
    ),
    (
        "design",
        "DESIGN.md",
        "binary-served",
        "523168903e1ae2d22951da0a4e54336369b3e2d17281fccc2e7561876011aa3c",
    ),
    (
        "schema",
        "backlog",
        "maestro.card.v1",
        "fda5556f0f296a95d3e9b6213fa2f2dc72f79d59efb5326d6ff4c325abebd663",
    ),
    (
        "schema",
        "card",
        "maestro.card.v1",
        "3bf356be6923e4027752015ce43515b5ad1bedaad15b054e1348b8fce18bfc4e",
    ),
    (
        "schema",
        "decision",
        "maestro.card.v1",
        "15bae87c3fd9b7200454078480e1f14cd06a79d3064e48c15eb12ce42de916f7",
    ),
    (
        "schema",
        "feature",
        "maestro.feature.v2",
        "aa696177f2727c94b339b8fcbb45ba3b10beec5b2552622570278149efea159d",
    ),
    (
        "schema",
        "harness",
        "maestro.harness.v1",
        "a570dbd3acad8e22ec644f17c0e5602e2440f0b0d247be73bd6125cd25415cf4",
    ),
    (
        "schema",
        "install",
        "maestro.install_lock.v1",
        "e9ff23c09bcea690c67446e7a0efabfbc36949f4596875b46ef21c8b83942329",
    ),
    (
        "schema",
        "progress",
        "maestro.progress.v1",
        "b57ce44a7992203d04a958463c66d6014e1d201eae4ad45c4aab790111fe71ca",
    ),
    (
        "schema",
        "proof",
        "maestro.verification.v1",
        "5122fac7ed7f4e40fcd122eb8e47da895d58a851fa62e47443414da64f799a6a",
    ),
    (
        "schema",
        "run-event",
        "maestro.event.v1",
        "d07495c2b9539dbc264fe0361bdc5a3919c495cbaa90e059b30db83305ccb190",
    ),
    (
        "schema",
        "run-evidence",
        "maestro.run_evidence.v1",
        "66bae6cc9fe317881dc0dfa27793108b9ea0f37609462fe60b039e0328119d98",
    ),
    (
        "schema",
        "task",
        "maestro.task.v2",
        "21d90854c3fede0f13f2ec8502ee7abd0e77940757c9c03d579f095518ec8916",
    ),
];

/// Collect every file under an embedded dir, with paths relative to `root`.
fn collect_embedded_files(
    dir: &'static Dir<'static>,
    root: &'static Dir<'static>,
) -> Vec<(&'static str, &'static [u8])> {
    let mut files: Vec<(&'static str, &'static [u8])> = dir
        .files()
        .map(|file| {
            let relative = file
                .path()
                .strip_prefix(root.path())
                .ok()
                .and_then(|path| path.to_str())
                .expect("invariant: an embedded file lives under its root with a UTF-8 path");
            (relative, file.contents())
        })
        .collect();
    for subdir in dir.dirs() {
        files.extend(collect_embedded_files(subdir, root));
    }
    files
}

/// The embedded schema pack directory for one artifact family.
fn schema_pack_dir(family: &str) -> Option<&'static Dir<'static>> {
    SCHEMAS_DIR
        .dirs()
        .find(|dir| dir.path().file_name().and_then(|name| name.to_str()) == Some(family))
}

/// Hash a resource's files: each `(relative path, bytes)`, sorted by path, each
/// length-prefixed so no separator can be forged by a path or byte payload (it
/// matters once a resource ships a binary asset). For a single-file resource the
/// list has one entry.
fn tree_hash(files: &[(&str, &[u8])]) -> String {
    let mut files: Vec<_> = files.iter().collect();
    files.sort_by_key(|(path, _)| *path);

    let mut buf = Vec::new();
    for (path, contents) in files {
        let path = path.as_bytes();
        buf.extend_from_slice(&(path.len() as u32).to_le_bytes());
        buf.extend_from_slice(path);
        buf.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        buf.extend_from_slice(contents);
    }
    sha256_hex(&buf)
}

#[test]
fn shipped_resource_trees_and_versions_match_the_recorded_guard() {
    for (group, name, version, hash) in RESOURCE_VERSION_GUARD {
        let (actual_hash, version_marker_present) = match group {
            "skill" => {
                let skill = skills()
                    .iter()
                    .find(|skill| skill.name == name)
                    .unwrap_or_else(|| panic!("recorded skill {name} is no longer shipped"));
                let files: Vec<(&str, &[u8])> = skill
                    .files
                    .iter()
                    // The generated reference/cli.md regenerates on any CLI
                    // change and has its own freshness gate; hashing it here
                    // would force a version bump for every flag edit.
                    .filter(|file| file.relative_path != "reference/cli.md")
                    .map(|file| (file.relative_path, file.contents))
                    .collect();
                (
                    tree_hash(&files),
                    skill.skill_md().contains(&format!("version: {version}")),
                )
            }
            "hook" => (
                tree_hash(&[(name, RECORD_SH.as_bytes())]),
                RECORD_SH.contains(&format!("# maestro:hook-version: {version}")),
            ),
            "harness" => (
                tree_hash(&[
                    (name, HARNESS_MD.as_bytes()),
                    ("RECOVERY.md", RECOVERY_MD.as_bytes()),
                ]),
                HARNESS_MD.contains(&format!("version: {version}")),
            ),
            "playbook" => {
                let files = collect_embedded_files(&PLAYBOOK_DIR, &PLAYBOOK_DIR);
                (tree_hash(&files), version == "binary-served")
            }
            "design" => {
                let files = collect_embedded_files(&DESIGN_DIR, &DESIGN_DIR);
                (tree_hash(&files), version == "binary-served")
            }
            "schema" => {
                let pack = schema_pack_dir(name)
                    .unwrap_or_else(|| panic!("recorded schema pack {name} is no longer shipped"));
                let files = collect_embedded_files(pack, pack);
                let current = files
                    .iter()
                    .find(|(path, _)| *path == "current.yaml")
                    .map(|(_, contents)| String::from_utf8_lossy(contents))
                    .unwrap_or_else(|| panic!("schema pack {name} is missing current.yaml"));
                (
                    tree_hash(&files),
                    current.contains(&format!("schema_version: {version}")),
                )
            }
            other => panic!("unknown resource group {other} in RESOURCE_VERSION_GUARD"),
        };

        assert_eq!(
            actual_hash, hash,
            "{group} {name} changed; bump its version if user-visible, then \
             re-record (version, tree-hash) in tests/resources_version_guard.rs",
        );
        assert!(
            version_marker_present,
            "{group} {name} must declare the recorded version {version}",
        );
    }
}

#[test]
fn every_recorded_guard_entry_maps_to_a_shipped_resource() {
    for (group, name, _, _) in RESOURCE_VERSION_GUARD {
        match group {
            "skill" => assert!(
                skills().iter().any(|skill| skill.name == name),
                "RESOURCE_VERSION_GUARD lists skill {name}, which is no longer shipped"
            ),
            // The hook script and harness protocol are fixed single-file
            // resources Maestro always ships. The playbook and design catalog
            // are fixed embedded trees served from the binary.
            "hook" | "harness" | "playbook" | "design" => {}
            "schema" => assert!(
                schema_pack_dir(name).is_some(),
                "RESOURCE_VERSION_GUARD lists schema pack {name}, which is no longer shipped"
            ),
            other => panic!("unknown resource group {other} in RESOURCE_VERSION_GUARD"),
        }
    }
}

#[test]
fn maestro_card_skill_keeps_explicit_unattended_loop_triggers() {
    let skill = skills()
        .iter()
        .find(|skill| skill.name == "maestro-card")
        .expect("maestro-card skill should ship");
    let mut body = String::new();
    for file in &skill.files {
        body.push_str(&String::from_utf8_lossy(file.contents));
        body.push('\n');
    }
    for phrase in [
        "use loop",
        "keep looping",
        "I am going away",
        "I am going to sleep",
        "work while I am away",
        "maestro loop work-lease --json",
    ] {
        assert!(
            body.contains(phrase),
            "maestro-card guidance must retain trigger phrase {phrase:?}"
        );
    }
}

#[test]
fn shipped_harness_and_skills_adopt_vnext_packet_routing() {
    let harness = HARNESS_MD.replace('\n', " ");
    assert!(
        harness.contains("maestro status --json")
            && harness.contains("canonical JSON request")
            && harness.contains("maestro packet read")
            && harness.contains("Never invent those authority facts")
            && harness.contains("typed refusal surfaces"),
        "harness must teach the vNext state/packet/authority split"
    );
    for phrase in [
        "authenticated host context",
        "expected Release/catalog identities",
        "legacy lifecycle recipe identifiers",
        "explicit current",
        "hidden stores",
        "hidden schedulers",
        "silent recipe mutation",
        "proof/QA bypass",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must explain the vNext routing boundary phrase {phrase:?}"
        );
    }
    for phrase in [
        "Design-to-card gate",
        "Am I coming from design or brainstorm?",
        "What card/feature owns this work?",
        "Is that card/feature handoff finalized and fresh?",
        "stop before creating Progress rows",
        "implicitly end the design phase",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain design-to-card gate phrase {phrase:?}"
        );
    }
    for phrase in [
        "Anti-MVP scope authority",
        "treat Full Durable Design as the scope authority",
        "Do not offer MVP, first-slice, or reduced product scope",
        "do not shrink the design target",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain anti-MVP scope authority phrase {phrase:?}"
        );
    }
    for phrase in [
        "Concrete repeatable form",
        "maestro task setup --task \"Map current behavior\" --task \"Implement scoped fix\" --task \"Verify\" --start",
        "Plain `--task` rows are serial by default",
        "use repeatable `--wave` rows",
        "follow-up `--then` rows",
        "maestro task setup --after <task-alias>=<dependency-alias-or-task-id>",
        "plan `after`/`blocked_by`",
        "blocked Progress successors under `blocked_next`",
        "finish and verify blockers first",
        "During implementation, keep running task notes with `maestro task note <task-id> \"<text>\"`",
        "Use `maestro note <card-id>` only for card-store",
        "decisions not in the handoff/spec",
        "scope or acceptance, amend the owning Feature/Card contract",
        "Canonical work readiness is `maestro ready`",
        "task-wave projection from the Task DAG",
        "Wave 1 / `parallel_wave` rows are independent executable tasks",
        "Use subagents or worktrees for that fan-out",
        "the orchestrator still owns shared Maestro store writes",
        "bounded blocked-next frontier",
        "no retired successor route creates a second scheduler",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain Ready V2 routing phrase {phrase:?}"
        );
    }
    assert!(
        harness.contains("bounded design mandate")
            && harness.contains("subagents/advisors provide evidence only")
            && harness.contains("control returns to the owning design session"),
        "harness must preserve delegated design authority"
    );
    for phrase in [
        "Loop readiness is native evidence",
        "maestro loop validate <pattern>",
        "blocked_from_next_level",
        "Do not claim L3 unattended",
        "External schedulers stay external",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain loop readiness evidence phrase {phrase:?}"
        );
    }
    for phrase in [
        "Native harness layer route",
        "`maestro intake`",
        "`maestro capability`",
        "`maestro maturity`",
        "`maestro install --dry-run`",
        "`maestro sync --dry-run`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must teach native layer routing phrase {phrase:?}"
        );
    }

    let design = shipped_skill_body("maestro-design").replace('\n', " ");
    assert!(
        design.contains("Routing checkpoint")
            && design.contains("maestro status --json")
            && design.contains("maestro packet read")
            && design.contains("Retired successor routes")
            && design.contains("explicit current Feature and Decision verbs")
            && design.contains("perceive -> choose -> act"),
        "maestro-design must adopt vNext packet routing"
    );
    assert!(
        design.contains("bounded design mandate")
            && design.contains("subagents/advisors provide evidence only")
            && design.contains("control returns to the owning design session"),
        "maestro-design must explain delegated design authority"
    );
    assert!(
        design.contains("Before technical forks, decide scope depth")
            && design.contains("Full Durable Design")
            && design.contains("Scope target and implementation staging are separate")
            && design.contains("Anti-MVP scope authority")
            && design.contains("Do not offer MVP, first-slice, or reduced product scope")
            && design.contains("Full target: the complete system being designed")
            && design.contains("Deferred work: only implementation sequencing"),
        "maestro-design must separate full design scope from implementation staging and reject MVP by default"
    );
    for phrase in [
        "canonical JSON request",
        "authenticated host context",
        "Never invent those authority facts",
        "legacy lifecycle recipe identifiers",
        "silent recipe mutation",
    ] {
        assert!(
            design.contains(phrase),
            "maestro-design must explain vNext routing boundary phrase {phrase:?}"
        );
    }
    for phrase in [
        "When designing loop automation",
        "native Maestro pattern packs",
        "readiness target (L0 draft, L1 report, L2 assisted, or L3 unattended)",
        "cadence/max-attempts/max-subagents/denylist/budget/kill-switch/connector",
        "Do not design a separate daemon",
    ] {
        assert!(
            design.contains(phrase),
            "maestro-design must retain loop readiness design phrase {phrase:?}"
        );
    }
    for phrase in [
        "no-fork edge sweep",
        "edge pressure",
        "Material unknowns reopen a fork",
        "Only a clean sweep reaches the explicit build-approval gate",
        "bounded edge sweep chained to Maestro's shipped Unknowns Lens",
        "compare every remaining `unknown_gap` against locked decisions",
        "edge-case pressure: what each option could miss",
        "No forks remain. Edge sweep found no material unresolved choices. Waiting for explicit build approval.",
    ] {
        assert!(
            design.contains(phrase),
            "maestro-design must retain no-fork Unknowns Lens sweep phrase {phrase:?}"
        );
    }
    assert!(
        design.contains("domain model")
            && design.contains("reference/domain-model.md")
            && design.contains("feature design")
            && design.contains("maestro decision")
            && design.contains("maestro grep"),
        "maestro-design must retain the domain-modeling branch"
    );
    assert!(
        design.contains("grill me")
            && design.contains("reference/grilling.md")
            && design.contains("one question at a time")
            && design.contains("Grill With Docs"),
        "maestro-design must retain the grilling branch"
    );
    assert!(
        design.contains("PRD synthesis")
            && design.contains("reference/prd.md")
            && design.contains("ready-for-agent")
            && design.contains("reference/deepening-candidate.md")
            && design.contains("Module")
            && design.contains("seam"),
        "maestro-design must retain PRD synthesis and deepening-candidate branches"
    );

    let audit = shipped_skill_body("maestro-audit").replace('\n', " ");
    assert!(
        audit.contains("Routing checkpoint")
            && audit.contains("maestro status --json")
            && audit.contains("maestro packet read")
            && audit.contains("Retired successor routes")
            && audit.contains("perceive -> choose -> act"),
        "maestro-audit must adopt vNext packet routing"
    );
    assert!(
        audit.contains("architecture review")
            && audit.contains("deepening opportunities")
            && audit.contains("reference/architecture-review.md")
            && audit.contains("architecture-review-<timestamp>.html")
            && audit.contains("Top recommendation")
            && audit.contains("locked-decision conflicts")
            && audit.contains("maestro grep"),
        "maestro-audit must retain the architecture review branch"
    );
    for phrase in [
        "canonical JSON request",
        "authenticated host context",
        "Never invent those authority facts",
        "legacy lifecycle recipe identifiers",
        "silent recipe mutation",
    ] {
        assert!(
            audit.contains(phrase),
            "maestro-audit must explain vNext routing boundary phrase {phrase:?}"
        );
    }

    let card = shipped_skill_body("maestro-card").replace('\n', " ");
    for phrase in [
        "Routing checkpoint",
        "maestro status --json",
        "maestro packet read",
        "Retired successor routes",
        "explicit current",
        "choose-phase helper",
        "not a scheduler, daemon, queue, worker launcher, executor",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must keep lifecycle recipe checkpoint phrase {phrase:?}"
        );
    }
    for phrase in [
        "canonical JSON request",
        "authenticated host context",
        "Never invent those authority facts",
        "hidden stores",
        "proof/QA bypass",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must explain loop intelligence boundary phrase {phrase:?}"
        );
    }
    for phrase in [
        "Loop readiness is an evidence gate",
        "For production loop patterns",
        "blocked_from_next_level",
        "Do not claim L3",
        "External schedulers stay external",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must retain loop readiness evidence phrase {phrase:?}"
        );
    }
    for phrase in [
        "Phase 0: design-to-card gate",
        "Before `task setup`",
        "Am I coming from design or brainstorm?",
        "What card/feature owns this work?",
        "Is that card/feature handoff finalized and fresh?",
        "Progress rows cannot be used",
        "Record implementation discoveries with `maestro task note <task-id> \"<text>\"`",
        "use `maestro note <card-id>` only for card-store notes",
        "plan changes, tradeoffs, gotchas, risks, and follow-up work",
        "Scope or acceptance changes still require Feature/Card contract amendment",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must retain design-to-card gate phrase {phrase:?}"
        );
    }
    for phrase in [
        "Use `maestro_ready` for task-wave orientation",
        "Use `maestro_card_ready`",
        "only for explicit legacy/card-board work",
        "executable `maestro ready <feature>` parallel wave",
        "ship gate -> ship",
        "It is not a queue, scheduler, executor, or hidden gate loop",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must retain Ready V2 routing phrase {phrase:?}"
        );
    }

    let setup = shipped_skill_body("maestro-setup").replace('\n', " ");
    for phrase in [
        "Routing checkpoint",
        "maestro status --json",
        "maestro packet read",
        "Retired successor routes",
        "explicit Maestro verbs",
    ] {
        assert!(
            setup.contains(phrase),
            "maestro-setup must keep loop-first routing phrase {phrase:?}"
        );
    }
    for phrase in [
        "canonical JSON request",
        "authenticated host context",
        "Never invent those authority facts",
        "hidden schedulers",
    ] {
        assert!(
            setup.contains(phrase),
            "maestro-setup must explain loop intelligence boundary phrase {phrase:?}"
        );
    }

    let ask = shipped_skill_body("ask-maestro").replace('\n', " ");
    for phrase in [
        "maestro status --json",
        "maestro packet read",
        "Retired successor routes",
        "Never invent those authority facts",
        "hidden stores",
    ] {
        assert!(
            ask.contains(phrase),
            "ask-maestro must explain vNext routing boundary phrase {phrase:?}"
        );
    }
}

#[test]
fn shipped_guidance_uses_vnext_packet_projection_and_refuses_legacy_routing() {
    let skill_names = [
        "ask-maestro",
        "maestro-audit",
        "maestro-card",
        "maestro-design",
        "maestro-setup",
    ];
    let mut bodies = vec![("HARNESS.md", HARNESS_MD.replace('\n', " "))];
    for name in skill_names {
        let body = skills()
            .iter()
            .find(|skill| skill.name == name)
            .unwrap_or_else(|| panic!("{name} should ship"))
            .skill_md()
            .replace('\n', " ");
        bodies.push((name, body));
    }
    for (name, body) in bodies {
        for phrase in [
            "maestro status --json",
            "canonical JSON request",
            "authenticated host context",
            "expected Release/catalog identities",
            "maestro packet read",
            "legacy lifecycle recipe identifiers",
        ] {
            assert!(body.contains(phrase), "{name} is missing {phrase:?}");
        }
        assert!(
            !body.contains("Invoke `maestro loop next")
                && !body.contains("run `maestro loop next")
                && !body.contains("Use `maestro loop next"),
            "{name} still recommends a retired successor route"
        );
    }
}

#[test]
fn native_layer_guidance_lives_in_targeted_teaching_surfaces() {
    let harness = normalize_markdown(HARNESS_MD);
    let design = normalize_markdown(&shipped_skill_md("maestro-design"));
    let card = normalize_markdown(&shipped_skill_body("maestro-card"));
    let audit = normalize_markdown(&shipped_skill_md("maestro-audit"));
    let setup = normalize_markdown(&shipped_skill_md("maestro-setup"));

    for phrase in [
        "Native harness layer route",
        "`maestro intake`",
        "`maestro capability`",
        "`maestro maturity`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must teach native layer phrase {phrase:?}"
        );
    }
    for phrase in [
        "Native harness layer design route",
        "`maestro intake`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            design.contains(phrase),
            "maestro-design SKILL.md must teach native layer phrase {phrase:?}"
        );
    }
    for phrase in [
        "Native harness layer work route",
        "`maestro intake`",
        "`maestro capability`",
        "`maestro maturity`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card SKILL.md must teach native layer phrase {phrase:?}"
        );
    }
    for phrase in [
        "Native harness layer audit route",
        "`maestro capability`",
        "`maestro maturity`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            audit.contains(phrase),
            "maestro-audit SKILL.md must teach native layer phrase {phrase:?}"
        );
    }
    for phrase in [
        "Native harness layer setup route",
        "`maestro install --dry-run`",
        "`maestro sync --dry-run`",
        "`maestro capability`",
        "`maestro maturity`",
        "Generated CLI references prove command shape; Harness and targeted skills teach the workflow.",
    ] {
        assert!(
            setup.contains(phrase),
            "maestro-setup SKILL.md must teach native layer phrase {phrase:?}"
        );
    }
}

#[test]
fn shipped_guidance_routes_weak_context_to_maestro_research_without_harness_sprawl() {
    let harness = normalize_markdown(HARNESS_MD);
    let design = normalize_markdown(&shipped_skill_md("maestro-design"));

    for phrase in [
        "zero-context, unfamiliar-domain, externally pasted, stakeholder-heavy, or hosting-unclear ideas",
        "route through `maestro-research` before `maestro-design`",
        "fresh `research.md`, an explicit skip receipt, or clearly settled context recorded with evidence",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain thin research router phrase {phrase:?}"
        );
    }
    for phrase in [
        "maestro-research",
        "fresh `research.md`",
        "explicit skip receipt",
        "clearly settled context recorded with evidence",
    ] {
        assert!(
            design.contains(phrase),
            "maestro-design must retain research entry gate phrase {phrase:?}"
        );
    }
    for forbidden in [
        "Research Status:",
        "Sales Copilot Regression",
        "blocking unknowns are zero",
    ] {
        assert!(
            !harness.contains(forbidden),
            "harness must stay router-only and exclude research contract phrase {forbidden:?}"
        );
    }
}

#[test]
fn repo_local_harness_copy_retains_shipped_research_router() {
    let local = normalize_markdown(
        &fs::read_to_string(".maestro/harness/HARNESS.md")
            .expect("repo-local harness copy should be readable"),
    );

    for phrase in [
        "version: 1.29.25",
        "zero-context, unfamiliar-domain, externally pasted, stakeholder-heavy, or hosting-unclear ideas",
        "route through `maestro-research` before `maestro-design`",
        "fresh `research.md`, an explicit skip receipt, or clearly settled context recorded with evidence",
        "`maestro research check <card-id>`",
        "risky-skipped, or hosting-incompatible",
    ] {
        assert!(
            local.contains(phrase),
            "repo-local harness copy must retain research router phrase {phrase:?}"
        );
    }
}

#[test]
fn maestro_witness_skill_teaches_close_receipt_contract() {
    let witness = shipped_skill_body("maestro-witness").replace('\n', " ");
    for phrase in [
        "witness.md",
        "advisor.md",
        "Gate: APPROVED",
        "witness conductor",
        "advisor independence",
        "Auto-invoke a fresh-context advisor subagent",
        "Human review is required only when the risk tier",
        "The conductor must not invent approval",
        "independent_session: true",
        "risk_tier: T0",
        "risk_tier: T1",
        "risk_tier: T2",
        "risk_tier: T3",
        "demo_waived: true",
        "expert_escalation: satisfied",
        "contract_ref",
        "proof_ref",
        "qa_ref",
        "tree_ref",
        "Do not paste large code dumps",
        "audit is backlog-only",
    ] {
        assert!(
            witness.contains(phrase),
            "maestro-witness must teach close receipt phrase {phrase:?}"
        );
    }
}

#[test]
fn lifecycle_guidance_routes_close_through_witness_without_replacing_proof() {
    let harness = normalize_markdown(HARNESS_MD);
    let card = normalize_markdown(&shipped_skill_body("maestro-card"));
    let audit = normalize_markdown(&shipped_skill_md("maestro-audit"));

    for phrase in [
        "post-implementation close witness",
        "`maestro-witness`",
        "after task proof, `maestro feature verify`, and QA slice evidence",
        "witness does not replace task proof, feature verify, or QA",
        "auto-invoked fresh-context subagent controlled by the main session",
        "human review or demo is required only when risk tier, policy, tool boundary, or explicit user direction demands it",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must route close through witness phrase {phrase:?}"
        );
    }

    for phrase in [
        "`maestro-witness` -> `maestro feature close`",
        "after [qa-slice.md](qa-slice.md)",
        "does not replace `maestro feature verify`, task proof, or QA",
        "Routine T1 close may satisfy the advisor receipt with an auto-invoked fresh-context subagent controlled by the main session",
    ] {
        assert!(
            card.contains(phrase),
            "maestro-card must route close through witness phrase {phrase:?}"
        );
    }

    for phrase in [
        "Audit findings are backlog-only during witness sign-off",
        "`maestro harness propose`",
        "do not become close blockers unless they invalidate the accepted contract, proof, QA, or risk-tier policy",
    ] {
        assert!(
            audit.contains(phrase),
            "maestro-audit must preserve witness audit-boundary phrase {phrase:?}"
        );
    }
}

fn shipped_skill_body(name: &str) -> String {
    let skill = skills()
        .iter()
        .find(|skill| skill.name == name)
        .unwrap_or_else(|| panic!("{name} skill should ship"));
    let mut body = String::new();
    for file in &skill.files {
        body.push_str(&String::from_utf8_lossy(file.contents));
        body.push('\n');
    }
    body
}

fn shipped_skill_md(name: &str) -> String {
    let skill = skills()
        .iter()
        .find(|skill| skill.name == name)
        .unwrap_or_else(|| panic!("{name} skill should ship"));
    skill.skill_md().to_owned()
}

fn normalize_markdown(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn every_shipped_schema_pack_is_recorded_in_the_guard() {
    for dir in SCHEMAS_DIR.dirs() {
        let family = dir
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("invariant: an embedded schema pack directory has a UTF-8 name");
        assert!(
            RESOURCE_VERSION_GUARD
                .iter()
                .any(|(group, name, _, _)| *group == "schema" && *name == family),
            "shipped schema pack {family} is missing from RESOURCE_VERSION_GUARD",
        );
    }
}

#[test]
fn shipped_playbook_tree_is_recorded_in_the_guard() {
    assert!(
        RESOURCE_VERSION_GUARD
            .iter()
            .any(|(group, name, _, _)| *group == "playbook" && *name == "PLAYBOOK.md"),
        "shipped playbook tree is missing from RESOURCE_VERSION_GUARD",
    );
}

#[test]
fn shipped_design_tree_is_recorded_in_the_guard() {
    assert!(
        RESOURCE_VERSION_GUARD
            .iter()
            .any(|(group, name, _, _)| *group == "design" && *name == "DESIGN.md"),
        "shipped design tree is missing from RESOURCE_VERSION_GUARD",
    );
}

#[test]
fn every_shipped_skill_is_recorded_in_the_guard() {
    for skill in skills() {
        assert!(
            RESOURCE_VERSION_GUARD
                .iter()
                .any(|(group, name, _, _)| *group == "skill" && *name == skill.name),
            "shipped skill {} is missing from RESOURCE_VERSION_GUARD",
            skill.name
        );
    }
}

#[test]
fn ask_maestro_routes_to_the_shipped_skill_family() {
    let ask = shipped_skill_body("ask-maestro").replace('\n', " ");
    for phrase in [
        "maestro-design",
        "maestro-card",
        "maestro-setup",
        "maestro-audit",
        "maestro status",
        "maestro loop next",
        "maestro task setup",
        "conflict-handoff",
    ] {
        assert!(
            ask.contains(phrase),
            "ask-maestro must retain routing phrase {phrase:?}"
        );
    }
}

#[test]
fn shipped_guidance_preserves_decisionset_anti_compression_rule() {
    let harness = HARNESS_MD.replace('\n', " ");
    let design = shipped_skill_body("maestro-design").replace('\n', " ");
    let card = shipped_skill_body("maestro-card").replace('\n', " ");
    let ask = shipped_skill_body("ask-maestro").replace('\n', " ");

    for (name, body) in [
        ("HARNESS.md", harness.as_str()),
        ("maestro-design", design.as_str()),
        ("maestro-card", card.as_str()),
        ("ask-maestro", ask.as_str()),
    ] {
        for phrase in [
            "lock all",
            "all-recommendations",
            "DecisionSet",
            "separate child decisions",
        ] {
            assert!(
                body.contains(phrase),
                "{name} must retain anti-compression phrase {phrase:?}"
            );
        }
    }

    for phrase in [
        "maestro decision set draft",
        "maestro decision set lock",
        "maestro decision audit --compressed",
        "maestro decision set repair",
    ] {
        assert!(
            harness.contains(phrase),
            "harness must retain DecisionSet repair command phrase {phrase:?}"
        );
    }
}
