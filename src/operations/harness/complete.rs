use std::fs;

use anyhow::Result;
use serde::Serialize;

use crate::domain::card;
use crate::domain::card::schema::CardType;
use crate::domain::install::{InstallLock, InstallState};
use crate::domain::run::{self, Presence};
use crate::domain::task::{self, TaskState};
use crate::domain::{decisions, memory};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::session::agent_runtime_from_env;
use crate::foundation::core::time::utc_now_timestamp;
use crate::operations::memory as memory_ops;

use super::{load_config, propose::over_threshold_items};

const COMPLETE_HARNESS_SCHEMA: &str = "maestro.complete_harness.v1";

#[derive(Clone, Debug, Serialize)]
pub struct CompleteHarnessReadout {
    pub schema: String,
    pub status: String,
    pub observability: ObservabilityReadout,
    pub hook_trace: HookTraceReadout,
    pub runtime_boundary: RuntimeBoundaryReadout,
    pub security_gates: SecurityGateReadout,
    pub guardrails: GuardrailReadout,
    pub scheduler: SchedulerReadout,
    pub proof_matrix: Vec<HarnessProofMatrixRow>,
    pub signals: Vec<HarnessSignal>,
}

impl CompleteHarnessReadout {
    pub fn summary_line(&self) -> String {
        format!(
            "harness: observability/liveness {} (harness={}, active_sessions={}, stale_sessions={}, proof_gaps={}, friction={})",
            self.observability.status,
            self.observability.harness_protocol,
            self.observability.active_sessions,
            self.observability.stale_sessions,
            self.observability.proof_gap_tasks,
            self.observability.recurring_friction_items
        )
    }

    pub fn hook_trace_summary_line(&self) -> String {
        format!(
            "harness: hook/trace {} (wiring={}, installed_agents={}, events={}, card_touch={}, task_proof={})",
            self.hook_trace.status,
            self.hook_trace.hook_wiring,
            self.hook_trace.installed_agents,
            self.hook_trace.recorded_events,
            self.hook_trace.card_touch_events,
            self.hook_trace.task_proof_events
        )
    }

    pub fn hook_trace_check_detail(&self) -> String {
        format!(
            "wiring={} installed_agents={} supported_events={} events={} card_touch={} task_proof={}",
            self.hook_trace.hook_wiring,
            self.hook_trace.installed_agents,
            self.hook_trace.supported_events,
            self.hook_trace.recorded_events,
            self.hook_trace.card_touch_events,
            self.hook_trace.task_proof_events
        )
    }

    pub fn runtime_summary_line(&self) -> String {
        format!(
            "harness: runtime/tool {} (stack={}, agent={}, installed_agents={}, cli={}, mcp={}, provider_model={})",
            self.runtime_boundary.status,
            self.runtime_boundary.stack_kind,
            self.runtime_boundary
                .current_agent_runtime
                .as_deref()
                .unwrap_or("unknown"),
            self.runtime_boundary.installed_agents,
            self.runtime_boundary.cli_surface,
            self.runtime_boundary.mcp_surface,
            self.runtime_boundary.provider_model.status
        )
    }

    pub fn runtime_check_detail(&self) -> String {
        format!(
            "stack={} verify_commands={} agent={} installed_agents={} cli={} mcp={} provider_model={}",
            self.runtime_boundary.stack_kind,
            self.runtime_boundary.verify_commands,
            self.runtime_boundary
                .current_agent_runtime
                .as_deref()
                .unwrap_or("unknown"),
            self.runtime_boundary.installed_agents,
            self.runtime_boundary.cli_surface,
            self.runtime_boundary.mcp_surface,
            self.runtime_boundary.provider_model.status
        )
    }

    pub fn security_summary_line(&self) -> String {
        format!(
            "harness: security gates {} (classes={}, risky_tasks={}, blocked={}, verified={}, qa_artifacts={}, decisions={})",
            self.security_gates.status,
            self.security_gates.classes.len(),
            self.security_gates.declared_risky_tasks,
            self.security_gates.blocked_risky_tasks,
            self.security_gates.verified_risky_tasks,
            self.security_gates.qa_artifacts,
            self.security_gates.decision_records
        )
    }

    pub fn security_check_detail(&self) -> String {
        let class_ids = self
            .security_gates
            .classes
            .iter()
            .map(|class| class.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "classes={} class_ids={} risky_tasks={} unclassified={} blocked={} verified={} proof_path={} waiver_path={} block_path={} qa_artifacts={} decisions={}",
            self.security_gates.classes.len(),
            class_ids,
            self.security_gates.declared_risky_tasks,
            self.security_gates.unclassified_risk_tasks,
            self.security_gates.blocked_risky_tasks,
            self.security_gates.verified_risky_tasks,
            self.security_gates.proof_path,
            self.security_gates.waiver_path,
            self.security_gates.block_path,
            self.security_gates.qa_artifacts,
            self.security_gates.decision_records
        )
    }

    pub fn guardrail_summary_line(&self) -> String {
        format!(
            "harness: guardrails {} (rules={}, interventions={}, candidates={}, promoted={}, rejected={}, scorer_receipts={}, task_checks={})",
            self.guardrails.status,
            self.guardrails.rules.len(),
            self.guardrails.intervention_events,
            self.guardrails.candidate_rules,
            self.guardrails.promoted_rules,
            self.guardrails.rejected_rules,
            self.guardrails.scorer_receipts,
            self.guardrails.task_check_rules
        )
    }

    pub fn guardrail_check_detail(&self) -> String {
        let rule_ids = self
            .guardrails
            .rules
            .iter()
            .map(|rule| rule.rule.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "rules={} rule_ids={} interventions={} candidates={} gated={} promoted={} rejected={} scorer_receipts={} task_checks={} agents_harness_sources={} lifecycle={}",
            self.guardrails.rules.len(),
            rule_ids,
            self.guardrails.intervention_events,
            self.guardrails.candidate_rules,
            self.guardrails.gated_rules,
            self.guardrails.promoted_rules,
            self.guardrails.rejected_rules,
            self.guardrails.scorer_receipts,
            self.guardrails.task_check_rules,
            self.guardrails.agents_harness_sources,
            self.guardrails.promotion_lifecycle.join(">")
        )
    }

    pub fn scheduler_summary_line(&self) -> String {
        self.scheduler.summary_line()
    }

    pub fn proof_matrix_summary_line(&self) -> String {
        let complete = self
            .proof_matrix
            .iter()
            .filter(|row| row.status == "complete")
            .count();
        let partial = self
            .proof_matrix
            .iter()
            .filter(|row| row.status == "partial")
            .count();
        let incomplete = self
            .proof_matrix
            .iter()
            .filter(|row| row.status == "incomplete")
            .count();
        format!(
            "harness: proof matrix rows={} complete={} partial={} incomplete={}",
            self.proof_matrix.len(),
            complete,
            partial,
            incomplete
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservabilityReadout {
    pub status: String,
    pub harness_protocol: String,
    pub active_sessions: usize,
    pub stale_sessions: usize,
    pub unconfirmed_sessions: usize,
    pub proof_gap_tasks: usize,
    pub recurring_friction_items: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct HookTraceReadout {
    pub status: String,
    pub hook_wiring: String,
    pub installed_agents: usize,
    pub pending_agents: usize,
    pub supported_events: usize,
    pub recorded_events: usize,
    pub card_touch_events: usize,
    pub task_proof_events: usize,
    pub skill_activation_events: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_event_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeBoundaryReadout {
    pub status: String,
    pub stack_kind: String,
    pub detected_by: Vec<String>,
    pub verify_commands: usize,
    pub package_boundary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_agent_runtime: Option<String>,
    pub installed_agents: usize,
    pub cli_version: String,
    pub cli_surface: String,
    pub mcp_surface: String,
    pub provider_model: ProviderModelReadout,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderModelReadout {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SecurityGateReadout {
    pub status: String,
    pub classes: Vec<SecurityGateClass>,
    pub declared_risky_tasks: usize,
    pub unclassified_risk_tasks: usize,
    pub blocked_risky_tasks: usize,
    pub verified_risky_tasks: usize,
    pub proof_path: String,
    pub waiver_path: String,
    pub block_path: String,
    pub qa_artifacts: usize,
    pub decision_records: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SecurityGateClass {
    pub id: String,
    pub label: String,
    pub enforcement_point: String,
    pub required_proof: String,
    pub waiver_or_block_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuardrailReadout {
    pub status: String,
    pub rules: Vec<GuardrailRule>,
    pub intervention_events: usize,
    pub candidate_rules: usize,
    pub gated_rules: usize,
    pub promoted_rules: usize,
    pub rejected_rules: usize,
    pub scorer_receipts: usize,
    pub task_check_rules: usize,
    pub agents_harness_sources: usize,
    pub promotion_lifecycle: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuardrailRule {
    pub rule: String,
    pub severity: String,
    pub source: String,
    pub evidence: Vec<String>,
    pub bypass_policy: String,
    pub lifecycle: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SchedulerReadout {
    pub status: String,
    pub stance: String,
    pub owner: String,
    pub heartbeat_source: String,
    pub heartbeat_events: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_heartbeat_at: Option<String>,
    pub active_sessions: usize,
    pub stale_sessions: usize,
    pub missed_runs: usize,
    pub dead_runs: usize,
    pub surfaces: Vec<String>,
    pub next_probe: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HarnessProofMatrixRow {
    pub gap: String,
    pub status: String,
    pub owning_surface: String,
    pub evidence: Vec<String>,
    pub inspect: Vec<String>,
    pub honest_limit: String,
}

impl SchedulerReadout {
    pub fn summary_line(&self) -> String {
        format!(
            "harness: scheduler {} (stance={}, owner={}, heartbeat_events={}, active_sessions={}, stale_sessions={}, missed_runs={}, dead_runs={})",
            self.status,
            self.stance,
            self.owner,
            self.heartbeat_events,
            self.active_sessions,
            self.stale_sessions,
            self.missed_runs,
            self.dead_runs
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HarnessSignal {
    pub id: String,
    pub label: String,
    pub status: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub inspect: Vec<String>,
}

pub fn complete_readout(paths: &MaestroPaths) -> Result<CompleteHarnessReadout> {
    complete_readout_for_roots(paths, std::slice::from_ref(paths))
}

pub fn complete_readout_for_roots(
    paths: &MaestroPaths,
    roots: &[MaestroPaths],
) -> Result<CompleteHarnessReadout> {
    let harness_protocol = if paths.harness_dir().join("HARNESS.md").is_file() {
        "present"
    } else {
        "missing"
    };

    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let proof_gap_tasks = task_entries
        .iter()
        .filter(|entry| entry.task.state == TaskState::NeedsVerification)
        .count();

    let now = utc_now_timestamp();
    let sessions = run::active_sessions_union(roots, &now)?;
    let stale_sessions = sessions
        .iter()
        .filter(|row| row.presence == Presence::Stale)
        .count();
    let unconfirmed_sessions = sessions
        .iter()
        .filter(|row| row.presence == Presence::Unconfirmed)
        .count();

    let recurring_friction_items = over_threshold_items(paths)?.len();
    let hook_trace = hook_trace_readout(paths)?;
    let runtime_boundary = runtime_boundary_readout(paths, hook_trace.installed_agents)?;
    let security_gates = security_gate_readout(paths, &task_entries);
    let guardrails = guardrail_readout(paths, &task_entries)?;
    let scheduler = scheduler_readout_from_sessions(paths, &sessions)?;
    let observability_status = if harness_protocol == "present"
        && stale_sessions == 0
        && proof_gap_tasks == 0
        && recurring_friction_items == 0
    {
        "complete"
    } else {
        "incomplete"
    };

    let observability = ObservabilityReadout {
        status: observability_status.to_string(),
        harness_protocol: harness_protocol.to_string(),
        active_sessions: sessions.len(),
        stale_sessions,
        unconfirmed_sessions,
        proof_gap_tasks,
        recurring_friction_items,
    };
    let mut signals = vec![HarnessSignal {
        id: "ac-1".to_string(),
        label: "observability_liveness".to_string(),
        status: observability.status.clone(),
        summary: format!(
            "harness={harness_protocol}; active_sessions={}; stale_sessions={stale_sessions}; proof_gap_tasks={proof_gap_tasks}; recurring_friction_items={recurring_friction_items}",
            sessions.len()
        ),
        evidence: vec![
            ".maestro/harness/HARNESS.md".to_string(),
            ".maestro/runs/*/events.jsonl".to_string(),
            ".maestro/cards/* task state".to_string(),
            ".maestro/harness/backlog.yaml or idea-card friction backlog".to_string(),
        ],
        inspect: vec![
            "maestro status".to_string(),
            "maestro resume".to_string(),
            "maestro active --all".to_string(),
            "maestro query run --json".to_string(),
        ],
    }];
    signals.push(HarnessSignal {
        id: "ac-2".to_string(),
        label: "hook_trace_coverage".to_string(),
        status: hook_trace.status.clone(),
        summary: format!(
            "wiring={}; installed_agents={}; supported_events={}; recorded_events={}; card_touch_events={}; task_proof_events={}",
            hook_trace.hook_wiring,
            hook_trace.installed_agents,
            hook_trace.supported_events,
            hook_trace.recorded_events,
            hook_trace.card_touch_events,
            hook_trace.task_proof_events
        ),
        evidence: vec![
            ".maestro/install-lock.yaml".to_string(),
            ".maestro/runs/*/events.jsonl".to_string(),
            "embedded/hooks/events.yaml".to_string(),
        ],
        inspect: vec![
            "maestro doctor".to_string(),
            "maestro install --agent <agent>".to_string(),
            "maestro hook record --event <event>".to_string(),
            "maestro task proof <task-id>".to_string(),
            "maestro query run --json".to_string(),
        ],
    });
    signals.push(HarnessSignal {
          id: "ac-3".to_string(),
        label: "runtime_provider_tool_boundary".to_string(),
        status: runtime_boundary.status.clone(),
        summary: format!(
            "stack={}; verify_commands={}; agent={}; installed_agents={}; cli={}; mcp={}; provider_model={}",
            runtime_boundary.stack_kind,
            runtime_boundary.verify_commands,
            runtime_boundary
                .current_agent_runtime
                .as_deref()
                .unwrap_or("unknown"),
            runtime_boundary.installed_agents,
            runtime_boundary.cli_surface,
            runtime_boundary.mcp_surface,
            runtime_boundary.provider_model.status
        ),
        evidence: vec![
            ".maestro/harness/harness.yml".to_string(),
            ".maestro/install-lock.yaml".to_string(),
            "MAESTRO_AGENT environment".to_string(),
            "maestro mcp serve CLI surface".to_string(),
        ],
        inspect: vec![
            "maestro doctor".to_string(),
            "maestro status --json".to_string(),
            "maestro resume".to_string(),
            "maestro sync --dry-run".to_string(),
          ],
      });
    signals.push(HarnessSignal {
        id: "ac-4".to_string(),
          label: "risky_action_security_gates".to_string(),
          status: security_gates.status.clone(),
          summary: format!(
              "classes={}; risky_tasks={}; unclassified={}; blocked={}; verified={}; proof_path={}; waiver_path={}; block_path={}",
              security_gates.classes.len(),
              security_gates.declared_risky_tasks,
              security_gates.unclassified_risk_tasks,
              security_gates.blocked_risky_tasks,
              security_gates.verified_risky_tasks,
              security_gates.proof_path,
              security_gates.waiver_path,
              security_gates.block_path
          ),
          evidence: vec![
              ".maestro/cards/* task risk".to_string(),
              ".maestro/cards/*/qa.md".to_string(),
              ".maestro/cards/* feature acceptance evidence".to_string(),
              ".maestro/cards/* decision context/lock".to_string(),
          ],
          inspect: vec![
              "maestro task show <task-id>".to_string(),
              "maestro task block <task-id> --reason \"<why>\"".to_string(),
              "maestro qa baseline <feature-id> --observed \"<evidence>\"".to_string(),
              "maestro decision new \"<title>\" --feature <feature-id>".to_string(),
              "maestro feature verify <feature-id> --prove <ac-id> --evidence \"<observed>\"".to_string(),
              "maestro feature verify <feature-id> --waive <ac-id> --reason \"<why>\"".to_string(),
            ],
        });
    signals.push(HarnessSignal {
        id: "ac-5".to_string(),
        label: "structured_guardrails".to_string(),
        status: guardrails.status.clone(),
        summary: format!(
            "rules={}; interventions={}; candidates={}; gated={}; promoted={}; rejected={}; scorer_receipts={}; task_checks={}; agents_harness_sources={}",
            guardrails.rules.len(),
            guardrails.intervention_events,
            guardrails.candidate_rules,
            guardrails.gated_rules,
            guardrails.promoted_rules,
            guardrails.rejected_rules,
            guardrails.scorer_receipts,
            guardrails.task_check_rules,
            guardrails.agents_harness_sources
        ),
        evidence: vec![
            ".maestro/runs/*/events.jsonl intervention events".to_string(),
            ".maestro/cards/*/memory/candidate.yml".to_string(),
            ".maestro/cards/*/memory/receipts/*.json".to_string(),
            ".maestro/cards/* task acceptance checks".to_string(),
            "AGENTS.md and .maestro/harness/HARNESS.md".to_string(),
        ],
        inspect: vec![
            "maestro event intervention --note \"<correction>\"".to_string(),
            "maestro memory create --from <source> --summary \"<rule>\"".to_string(),
            "maestro memory scorer <memory-id> --scorer-receipt <receipt>".to_string(),
            "maestro scorer run <memory-id>#gate.scorer_contract".to_string(),
            "maestro memory promote <memory-id> --plan|--apply".to_string(),
            "maestro task show <task-id>".to_string(),
            "maestro resume".to_string(),
        ],
    });
    signals.push(HarnessSignal {
        id: "ac-6".to_string(),
        label: "passive_scheduler_liveness".to_string(),
        status: scheduler.status.clone(),
        summary: format!(
            "stance={}; owner={}; heartbeat_events={}; active_sessions={}; stale_sessions={}; missed_runs={}; dead_runs={}",
            scheduler.stance,
            scheduler.owner,
            scheduler.heartbeat_events,
            scheduler.active_sessions,
            scheduler.stale_sessions,
            scheduler.missed_runs,
            scheduler.dead_runs
        ),
        evidence: vec![
            ".maestro/runs/*/events.jsonl event timestamps".to_string(),
            "maestro active session presence".to_string(),
            "maestro watch polling frame".to_string(),
        ],
        inspect: vec![
            "maestro loop work-lease --json".to_string(),
            "maestro next --json".to_string(),
            "maestro watch snapshot".to_string(),
            "maestro active --all".to_string(),
            "maestro query run --json".to_string(),
        ],
    });
    let proof_matrix = proof_matrix_rows(
        &observability,
        &hook_trace,
        &runtime_boundary,
        &security_gates,
        &guardrails,
        &scheduler,
    );

    Ok(CompleteHarnessReadout {
        schema: COMPLETE_HARNESS_SCHEMA.to_string(),
        status: aggregate_status(
            &observability.status,
            &hook_trace.status,
            &runtime_boundary.status,
            &security_gates.status,
            &guardrails.status,
        ),
        observability,
        hook_trace,
        runtime_boundary,
        security_gates,
        guardrails,
        scheduler,
        proof_matrix,
        signals,
    })
}

pub fn scheduler_readout(paths: &MaestroPaths) -> Result<SchedulerReadout> {
    scheduler_readout_for_roots(paths, std::slice::from_ref(paths))
}

pub fn scheduler_readout_for_roots(
    paths: &MaestroPaths,
    roots: &[MaestroPaths],
) -> Result<SchedulerReadout> {
    let now = utc_now_timestamp();
    let sessions = run::active_sessions_union(roots, &now)?;
    scheduler_readout_from_sessions(paths, &sessions)
}

pub fn scheduler_surface_line(paths: &MaestroPaths) -> Result<String> {
    Ok(scheduler_readout(paths)?.summary_line())
}

pub fn security_task_gate_line(task: &task::TaskRecord) -> Option<String> {
    let risk = task.risk.as_deref()?;
    let class_id = security_gate_class_id(risk)?;
    Some(format!(
        "security_gate: {class_id} (enforcement=task verify/complete + feature verify/close; proof=task --claim/--proof or feature --prove; waiver=feature verify --waive; block=maestro task block)"
    ))
}

pub fn security_feature_gate_line() -> &'static str {
    "harness: security gate path proof=feature verify --prove; waiver=feature verify --waive; block=maestro task block"
}

pub fn security_decision_gate_line() -> &'static str {
    "harness: security gate policy path decision=new/lock/supersede; enforcement remains task proof + feature close"
}

pub fn security_qa_gate_line() -> &'static str {
    "harness: security gate QA path captures required proof and risky-action regression evidence"
}

pub fn guardrail_decision_line() -> &'static str {
    "harness: guardrail rule=decision_policy_gate severity=medium source=decision evidence=context/lock/rejected-options bypass=supersede lifecycle=intervention>memory_candidate>promoted_or_rejected_rule"
}

pub fn guardrail_memory_line() -> &'static str {
    "harness: guardrail rule=memory_promotion_gate severity=high source=memory_candidate evidence=gate/risk/lifecycle bypass=memory-reject-or-stale lifecycle=intervention>memory_candidate>scorer_or_review_gate>promoted_or_rejected_rule"
}

pub fn guardrail_scorer_line() -> &'static str {
    "harness: guardrail rule=scorer_receipt_gate severity=high source=scorer_receipt evidence=passed-or-failed-receipt bypass=review-only-gate lifecycle=scorer_or_review_gate"
}

pub fn guardrail_task_check_line(task: &task::TaskRecord) -> Option<String> {
    let check_count = task.acceptance.checks.len();
    let has_verify_command = task.verify_command.is_some();
    if check_count == 0 && !has_verify_command {
        return None;
    }
    Some(format!(
        "harness: guardrail rule=task_check_verify_contract severity=medium source=task.acceptance evidence=checks:{check_count},verify_command:{has_verify_command} bypass=task-block-or-feature-waive lifecycle=task_check"
    ))
}

fn runtime_boundary_readout(
    paths: &MaestroPaths,
    installed_agents: usize,
) -> Result<RuntimeBoundaryReadout> {
    let config = load_config(paths)?;
    let (stack_kind, detected_by, verify_commands, package_boundary) = match config {
        Some(config) => {
            let stack_kind = stack_kind_label(&config.stack.kind);
            let package_boundary = package_boundary_for(stack_kind, &config.stack.verify);
            (
                stack_kind.to_string(),
                config.stack.detected_by,
                config.stack.verify.len(),
                package_boundary,
            )
        }
        None => (
            "undeclared".to_string(),
            Vec::new(),
            0,
            "undeclared".to_string(),
        ),
    };
    let current_agent_runtime = agent_runtime_from_env().map(str::to_string);
    let provider_model = ProviderModelReadout {
        status: "unverified".to_string(),
        provider: None,
        model: None,
        note: "provider/model is not locally verifiable unless a repo declares it separately"
            .to_string(),
    };
    let status = if stack_kind == "undeclared" || current_agent_runtime.is_none() {
        "missing"
    } else if provider_model.status == "unverified" {
        "partial"
    } else {
        "complete"
    };

    Ok(RuntimeBoundaryReadout {
        status: status.to_string(),
        stack_kind,
        detected_by,
        verify_commands,
        package_boundary,
        current_agent_runtime,
        installed_agents,
        cli_version: env!("MAESTRO_VERSION").to_string(),
        cli_surface: "available".to_string(),
        mcp_surface: "available".to_string(),
        provider_model,
    })
}

fn hook_trace_readout(paths: &MaestroPaths) -> Result<HookTraceReadout> {
    let lock = InstallLock::load(&paths.install_lock_file())?;
    let installed_agents = lock
        .agents
        .values()
        .filter(|agent| agent.state == InstallState::Committed)
        .count();
    let pending_agents = lock
        .agents
        .values()
        .filter(|agent| agent.state == InstallState::Pending)
        .count();
    let hook_wiring = if installed_agents > 0 {
        "installed"
    } else if pending_agents > 0 {
        "pending"
    } else {
        "missing"
    };

    let supported_events = run::hook_event_contract().shared_events().len();
    let mut recorded_events = 0;
    let mut card_touch_events = 0;
    let mut task_proof_events = 0;
    let mut skill_activation_events = 0;
    let mut latest_event: Option<(String, String)> = None;

    run::visit_managed_events(paths, |record| {
        let event = record.event();
        let kind = event
            .event_type()
            .or_else(|| event.alias_kind())
            .unwrap_or("<unknown>");
        recorded_events += 1;
        match kind {
            "card_touch" => card_touch_events += 1,
            "task_proof" => task_proof_events += 1,
            "skill_activation" | "SkillActivation" => skill_activation_events += 1,
            _ => {}
        }
        if let Some(ts) = event.timestamp()
            && latest_event
                .as_ref()
                .is_none_or(|(latest_ts, _)| ts > latest_ts.as_str())
        {
            latest_event = Some((ts.to_string(), kind.to_string()));
        }
        Ok(())
    })?;

    let status = if hook_wiring != "installed" {
        "missing"
    } else if recorded_events == 0 {
        "missing_evidence"
    } else if card_touch_events > 0 && task_proof_events > 0 {
        "complete"
    } else {
        "partial"
    };
    let (latest_event_at, latest_event) = latest_event
        .map(|(ts, kind)| (Some(ts), Some(kind)))
        .unwrap_or((None, None));

    Ok(HookTraceReadout {
        status: status.to_string(),
        hook_wiring: hook_wiring.to_string(),
        installed_agents,
        pending_agents,
        supported_events,
        recorded_events,
        card_touch_events,
        task_proof_events,
        skill_activation_events,
        latest_event,
        latest_event_at,
    })
}

fn security_gate_readout(
    paths: &MaestroPaths,
    task_entries: &[task::TaskEntry],
) -> SecurityGateReadout {
    let mut declared_risky_tasks = 0;
    let mut unclassified_risk_tasks = 0;
    let mut blocked_risky_tasks = 0;
    let mut verified_risky_tasks = 0;

    for entry in task_entries {
        let Some(risk) = entry.task.risk.as_deref() else {
            continue;
        };
        if security_gate_class_id(risk).is_some() {
            declared_risky_tasks += 1;
            if task::has_unresolved_blockers(&entry.task) {
                blocked_risky_tasks += 1;
            }
            if entry.task.state == TaskState::Verified {
                verified_risky_tasks += 1;
            }
        } else if !default_risk_label(risk) {
            unclassified_risk_tasks += 1;
        }
    }

    let status = if unclassified_risk_tasks == 0 {
        "complete"
    } else {
        "partial"
    };

    SecurityGateReadout {
        status: status.to_string(),
        classes: security_gate_classes(),
        declared_risky_tasks,
        unclassified_risk_tasks,
        blocked_risky_tasks,
        verified_risky_tasks,
        proof_path: "task complete --claim/--proof; task verify; feature verify --prove"
            .to_string(),
        waiver_path: "feature verify --waive --reason".to_string(),
        block_path: "task block --reason [--by decision|task|external]".to_string(),
        qa_artifacts: qa_artifact_count(paths),
        decision_records: decisions::list_tolerant(paths).len(),
    }
}

fn guardrail_readout(
    paths: &MaestroPaths,
    task_entries: &[task::TaskEntry],
) -> Result<GuardrailReadout> {
    let mut intervention_events = 0;
    run::visit_managed_events(paths, |record| {
        if record.event().is_event_type("intervention") {
            intervention_events += 1;
        }
        Ok(())
    })?;

    let mut candidate_rules = 0;
    let mut gated_rules = 0;
    let mut promoted_rules = 0;
    let mut rejected_rules = 0;
    let mut scorer_receipts = 0;
    let scan = card::query::scan_with_failures(paths)?;
    for (card, _) in scan.cards {
        if card.card_type != CardType::Memory {
            continue;
        }
        let Ok(candidate) = memory::validate_card(paths, &card) else {
            continue;
        };
        candidate_rules += 1;
        match candidate.memory.lifecycle {
            memory::MemoryLifecycle::Gated => gated_rules += 1,
            memory::MemoryLifecycle::Promoted => promoted_rules += 1,
            memory::MemoryLifecycle::Rejected => rejected_rules += 1,
            _ => {}
        }
        scorer_receipts += memory_ops::list_scorer_receipts(paths, &candidate.id)
            .map(|receipts| receipts.len())
            .unwrap_or(0);
    }

    let task_check_rules = task_entries
        .iter()
        .filter(|entry| {
            !entry.task.acceptance.checks.is_empty() || entry.task.verify_command.is_some()
        })
        .count();
    let agents_harness_sources = [
        paths.repo_root().join("AGENTS.md"),
        paths.harness_dir().join("HARNESS.md"),
    ]
    .iter()
    .filter(|path| path.is_file())
    .count();

    let promotion_lifecycle = vec![
        "intervention".to_string(),
        "memory_candidate".to_string(),
        "scorer_or_review_gate".to_string(),
        "promoted_or_rejected_rule".to_string(),
        "agents_or_harness_policy".to_string(),
    ];
    let rules = vec![
        GuardrailRule {
            rule: "agents_harness_operating_contract".to_string(),
            severity: "high".to_string(),
            source: "AGENTS.md + .maestro/harness/HARNESS.md".to_string(),
            evidence: vec![
                "AGENTS.md".to_string(),
                ".maestro/harness/HARNESS.md".to_string(),
                format!("present_sources={agents_harness_sources}/2"),
            ],
            bypass_policy:
                "explicit user instruction may override workflow guidance; destructive or permission-sensitive actions still require approval"
                    .to_string(),
            lifecycle: "agents_or_harness_policy".to_string(),
        },
        GuardrailRule {
            rule: "task_check_verify_contract".to_string(),
            severity: "medium".to_string(),
            source: "Task acceptance checks and per-task verify command".to_string(),
            evidence: vec![format!("task_check_rules={task_check_rules}")],
            bypass_policy:
                "missing proof blocks task verification; use task block or feature waiver for explicit exceptions"
                    .to_string(),
            lifecycle: "task_check".to_string(),
        },
        GuardrailRule {
            rule: "memory_promotion_gate".to_string(),
            severity: "high".to_string(),
            source: "Memory candidate gate/risk/lifecycle".to_string(),
            evidence: vec![
                format!("candidate_rules={candidate_rules}"),
                format!("gated_rules={gated_rules}"),
                format!("promoted_rules={promoted_rules}"),
                format!("rejected_rules={rejected_rules}"),
            ],
            bypass_policy:
                "gate.required controls review, scorer, scorer_and_review, or forbidden; rejected rules do not promote"
                    .to_string(),
            lifecycle: "memory_candidate>promoted_or_rejected_rule".to_string(),
        },
        GuardrailRule {
            rule: "scorer_receipt_gate".to_string(),
            severity: "high".to_string(),
            source: "Memory scorer receipts".to_string(),
            evidence: vec![format!("scorer_receipts={scorer_receipts}")],
            bypass_policy:
                "missing or failed scorer receipts block scorer-required promotion unless a review-only gate applies"
                    .to_string(),
            lifecycle: "scorer_or_review_gate".to_string(),
        },
        GuardrailRule {
            rule: "intervention_to_rule_lifecycle".to_string(),
            severity: "medium".to_string(),
            source: "Run intervention events and Memory promotion".to_string(),
            evidence: vec![format!("intervention_events={intervention_events}")],
            bypass_policy:
                "interventions are evidence only until promoted through Memory or rejected with review evidence"
                    .to_string(),
            lifecycle: promotion_lifecycle.join(">"),
        },
    ];
    let status = if agents_harness_sources == 2 {
        "complete"
    } else {
        "partial"
    };

    Ok(GuardrailReadout {
        status: status.to_string(),
        rules,
        intervention_events,
        candidate_rules,
        gated_rules,
        promoted_rules,
        rejected_rules,
        scorer_receipts,
        task_check_rules,
        agents_harness_sources,
        promotion_lifecycle,
    })
}

fn scheduler_readout_from_sessions(
    paths: &MaestroPaths,
    sessions: &[run::SessionActivity],
) -> Result<SchedulerReadout> {
    let active_sessions = sessions.len();
    let stale_sessions = sessions
        .iter()
        .filter(|row| row.presence == Presence::Stale)
        .count();
    let unconfirmed_sessions = sessions
        .iter()
        .filter(|row| row.presence == Presence::Unconfirmed)
        .count();
    let missed_runs = stale_sessions + unconfirmed_sessions;
    let dead_runs = stale_sessions;
    let mut heartbeat_events = 0;
    let mut latest_heartbeat_at: Option<String> = None;

    run::visit_managed_events(paths, |record| {
        let event = record.event();
        if let Some(ts) = event.timestamp() {
            heartbeat_events += 1;
            if latest_heartbeat_at
                .as_deref()
                .is_none_or(|latest| ts > latest)
            {
                latest_heartbeat_at = Some(ts.to_string());
            }
        }
        Ok(())
    })?;

    let status = if dead_runs > 0 {
        "degraded"
    } else if heartbeat_events > 0 || active_sessions > 0 {
        "observed"
    } else {
        "idle"
    };

    Ok(SchedulerReadout {
        status: status.to_string(),
        stance: "passive_local_first".to_string(),
        owner: "none".to_string(),
        heartbeat_source: "managed run event timestamps and active-session presence".to_string(),
        heartbeat_events,
        latest_heartbeat_at,
        active_sessions,
        stale_sessions,
        missed_runs,
        dead_runs,
        surfaces: vec![
            "loop".to_string(),
            "next".to_string(),
            "watch".to_string(),
            "active".to_string(),
            "query_run".to_string(),
        ],
        next_probe: "rerun maestro status, active --all, watch snapshot, or query run --json"
            .to_string(),
    })
}

fn proof_matrix_rows(
    observability: &ObservabilityReadout,
    hook_trace: &HookTraceReadout,
    runtime_boundary: &RuntimeBoundaryReadout,
    security_gates: &SecurityGateReadout,
    guardrails: &GuardrailReadout,
    scheduler: &SchedulerReadout,
) -> Vec<HarnessProofMatrixRow> {
    vec![
        HarnessProofMatrixRow {
            gap: "observability_liveness".to_string(),
            status: matrix_status(&observability.status),
            owning_surface: "status, resume, active, query-run".to_string(),
            evidence: vec![
                ".maestro/harness/HARNESS.md".to_string(),
                ".maestro/runs/*/events.jsonl".to_string(),
                ".maestro/cards task proof state".to_string(),
            ],
            inspect: vec![
                "maestro status --json".to_string(),
                "maestro resume".to_string(),
                "maestro active --all".to_string(),
                "maestro query run --json".to_string(),
            ],
            honest_limit:
                "local run events and task artifacts prove local visibility, not remote service health"
                    .to_string(),
        },
        HarnessProofMatrixRow {
            gap: "hook_trace_coverage".to_string(),
            status: matrix_status(&hook_trace.status),
            owning_surface: "doctor, install, hook, task-proof, query-run".to_string(),
            evidence: vec![
                ".maestro/install-lock.yaml".to_string(),
                ".maestro/runs/*/events.jsonl".to_string(),
                "embedded/hooks/events.yaml".to_string(),
            ],
            inspect: vec![
                "maestro doctor".to_string(),
                "maestro install --agent <agent>".to_string(),
                "maestro hook record --event <event>".to_string(),
                "maestro task proof <task-id>".to_string(),
            ],
            honest_limit:
                "coverage is complete only after installed hooks have emitted representative events"
                    .to_string(),
        },
        HarnessProofMatrixRow {
            gap: "runtime_provider_tool_boundary".to_string(),
            status: matrix_status(&runtime_boundary.status),
            owning_surface: "doctor, install, sync, status, resume".to_string(),
            evidence: vec![
                ".maestro/harness/harness.yml".to_string(),
                ".maestro/install-lock.yaml".to_string(),
                "MAESTRO_AGENT".to_string(),
                "maestro mcp serve CLI surface".to_string(),
            ],
            inspect: vec![
                "maestro doctor".to_string(),
                "maestro status --json".to_string(),
                "maestro resume".to_string(),
                "maestro sync --dry-run".to_string(),
            ],
            honest_limit:
                "provider/model remains unverified unless a repo declares it outside Maestro"
                    .to_string(),
        },
        HarnessProofMatrixRow {
            gap: "risky_action_security_gates".to_string(),
            status: matrix_status(&security_gates.status),
            owning_surface: "task, feature, qa, decision, doctor".to_string(),
            evidence: vec![
                ".maestro/cards task risk".to_string(),
                ".maestro/cards/*/qa.md".to_string(),
                ".maestro/cards feature acceptance evidence".to_string(),
                ".maestro/cards decision records".to_string(),
            ],
            inspect: vec![
                "maestro task show <task-id>".to_string(),
                "maestro feature verify <feature-id>".to_string(),
                "maestro qa baseline <feature-id> --observed <evidence>".to_string(),
                "maestro decision show <decision-id>".to_string(),
            ],
            honest_limit:
                "security gates classify and route local actions; they do not authorize external platforms"
                    .to_string(),
        },
        HarnessProofMatrixRow {
            gap: "structured_guardrails".to_string(),
            status: matrix_status(&guardrails.status),
            owning_surface: "decision, memory, scorer, task-check, AGENTS/HARNESS".to_string(),
            evidence: vec![
                ".maestro/runs intervention events".to_string(),
                ".maestro/cards/*/memory/candidate.yml".to_string(),
                ".maestro/cards/*/memory/receipts/*.json".to_string(),
                "AGENTS.md and .maestro/harness/HARNESS.md".to_string(),
            ],
            inspect: vec![
                "maestro memory list --all".to_string(),
                "maestro scorer list --memory <memory-id>".to_string(),
                "maestro task show <task-id>".to_string(),
                "maestro doctor".to_string(),
            ],
            honest_limit:
                "interventions become durable guardrails only after Memory promotion or explicit rejection"
                    .to_string(),
        },
        HarnessProofMatrixRow {
            gap: "passive_scheduler_liveness".to_string(),
            status: matrix_status(&scheduler.status),
            owning_surface: "loop, next, watch, active, query-run".to_string(),
            evidence: vec![
                ".maestro/runs event timestamps".to_string(),
                "active session presence".to_string(),
                "watch polling frame".to_string(),
            ],
            inspect: vec![
                "maestro loop work-lease --json".to_string(),
                "maestro next --json".to_string(),
                "maestro watch snapshot".to_string(),
                "maestro active --all".to_string(),
                "maestro query run --json".to_string(),
            ],
            honest_limit:
                "Maestro remains passive local-first substrate; it reports liveness but owns no daemon or cron scheduler"
                    .to_string(),
        },
    ]
}

fn matrix_status(status: &str) -> String {
    match status {
        "complete" | "observed" | "idle" => "complete",
        "missing" | "incomplete" => "incomplete",
        _ => "partial",
    }
    .to_string()
}

fn security_gate_classes() -> Vec<SecurityGateClass> {
    security_gate_class_specs()
        .iter()
        .map(|(id, label)| SecurityGateClass {
            id: (*id).to_string(),
            label: (*label).to_string(),
            enforcement_point: "task verify/complete plus feature verify/close".to_string(),
            required_proof: "task claim/proof, QA evidence, or feature acceptance evidence"
                .to_string(),
            waiver_or_block_path: "feature verify --waive or task block --reason".to_string(),
        })
        .collect()
}

fn security_gate_class_specs() -> &'static [(&'static str, &'static str)] {
    &[
        ("destructive_fs_git", "destructive filesystem/git"),
        ("dependency_version", "dependency/version"),
        ("schema_migration", "schema/migration"),
        ("secrets", "secrets"),
        ("external_side_effects", "external side effects"),
        ("release_publish_push", "release/publish/push"),
    ]
}

fn security_gate_class_id(risk: &str) -> Option<&'static str> {
    match normalize_risk(risk).as_str() {
        "destructive"
        | "destructive_fs"
        | "destructive_git"
        | "destructive_fs_git"
        | "destructive_filesystem_git"
        | "filesystem_git" => Some("destructive_fs_git"),
        "dependency"
        | "dependencies"
        | "version"
        | "dependency_version"
        | "dependency_versions"
        | "dependencies_versions" => Some("dependency_version"),
        "schema" | "migration" | "schema_migration" | "schema_migrations" => {
            Some("schema_migration")
        }
        "secret" | "secrets" => Some("secrets"),
        "external" | "external_side_effect" | "external_side_effects" | "network_side_effects" => {
            Some("external_side_effects")
        }
        "release"
        | "publish"
        | "push"
        | "release_publish"
        | "publish_push"
        | "release_publish_push" => Some("release_publish_push"),
        _ => None,
    }
}

fn default_risk_label(risk: &str) -> bool {
    matches!(
        normalize_risk(risk).as_str(),
        "" | "none" | "low" | "medium" | "high" | "critical"
    )
}

fn normalize_risk(risk: &str) -> String {
    risk.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn qa_artifact_count(paths: &MaestroPaths) -> usize {
    let cards_dir = paths.cards_dir();
    let Ok(entries) = fs::read_dir(cards_dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("qa.md").is_file())
        .count()
}

fn stack_kind_label(kind: &crate::domain::harness::StackKind) -> &'static str {
    match kind {
        crate::domain::harness::StackKind::Rust => "rust",
        crate::domain::harness::StackKind::TypeScriptNode => "type_script_node",
        crate::domain::harness::StackKind::Python => "python",
        crate::domain::harness::StackKind::Generic => "generic",
    }
}

fn package_boundary_for(stack_kind: &str, verify_commands: &[String]) -> String {
    match stack_kind {
        "rust" => "cargo".to_string(),
        "type_script_node" => "bun".to_string(),
        "python" => "python".to_string(),
        "generic"
            if verify_commands
                .iter()
                .any(|command| command.starts_with("make ")) =>
        {
            "make".to_string()
        }
        "generic" => "none".to_string(),
        _ => "undeclared".to_string(),
    }
}

fn aggregate_status(
    observability: &str,
    hook_trace: &str,
    runtime_boundary: &str,
    security_gates: &str,
    guardrails: &str,
) -> String {
    if observability == "complete"
        && hook_trace == "complete"
        && runtime_boundary == "complete"
        && security_gates == "complete"
        && guardrails == "complete"
    {
        "complete"
    } else if hook_trace == "missing"
        || observability == "incomplete"
        || runtime_boundary == "missing"
        || security_gates == "missing"
        || guardrails == "missing"
    {
        "incomplete"
    } else {
        "partial"
    }
    .to_string()
}
