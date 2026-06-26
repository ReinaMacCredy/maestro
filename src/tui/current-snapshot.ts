import type { DoctorCheck } from "@/infra/domain/status-types.js";
import type { FeatureStatus } from "@/shared/domain/legacy-mission.js";
import type { TaskStatus } from "@/shared/domain/task/index.js";
import type {
  AgentGridRow,
  DispatchQueueItem,
  EventStreamEntry,
  PrincipleEffectivenessRow,
  TaskBoardItem,
  TaskBoardSnapshot,
  TimelineMilestoneEntry,
} from "@/tui/state/screen-types.js";
import type {
  MissionControlConfigInspector,
  MissionControlConfigRow,
  MissionControlConfigTab,
  MissionControlEvent,
  MissionControlFeatureRow,
  MissionControlSnapshot,
  TaskPreviewPane,
} from "@/tui/state/types.js";

export interface RustMissionControlSnapshot {
  readonly schema: "maestro.mission_control.snapshot.v1";
  readonly mode: string;
  readonly repo: {
    readonly root: string;
    readonly branch?: string | null;
    readonly dirty: boolean;
    readonly code_other_dirty: number;
    readonly maestro_dirty: number;
  };
  readonly summary: {
    readonly features: number;
    readonly workable_cards: number;
    readonly ready: number;
    readonly active: number;
    readonly needs_verification: number;
    readonly blocked: number;
    readonly done: number;
    readonly live_sessions: number;
  };
  readonly features: readonly RustFeatureSnapshot[];
  readonly tasks: readonly RustTaskSnapshot[];
  readonly sessions: readonly RustSessionSnapshot[];
  readonly proof: {
    readonly needs_verification: number;
    readonly verified_or_done: number;
    readonly blocked: number;
    readonly proof_missing: number;
    readonly proof_failed: number;
    readonly proof_accepted: number;
    readonly proof_stale: number;
  };
  readonly config: {
    readonly preview_screens: readonly string[];
    readonly read_only: boolean;
    readonly source: string;
  };
}

interface RustFeatureSnapshot {
  readonly id: string;
  readonly title: string;
  readonly status: string;
  readonly total: number;
  readonly ready: number;
  readonly active: number;
  readonly needs_verification: number;
  readonly blocked: number;
  readonly done: number;
}

interface RustTaskSnapshot {
  readonly id: string;
  readonly title: string;
  readonly status: string;
  readonly state: string;
  readonly parent?: string | null;
  readonly claimed_by?: string | null;
}

interface RustSessionSnapshot {
  readonly session_id: string;
  readonly agent_runtime?: string | null;
  readonly mode?: string | null;
  readonly bound_card?: string | null;
  readonly last_action: string;
  readonly age_minutes: number;
  readonly presence: string;
}

const CONFIG_TABS: readonly MissionControlConfigTab[] = [
  "overview",
  "effective",
  "project",
  "global",
  "defaults",
  "plan",
  "doctor",
  "memory",
];

export function adaptRustSnapshot(snapshot: RustMissionControlSnapshot): MissionControlSnapshot {
  const now = Date.now();
  const taskRows = snapshot.tasks.length > 0
    ? snapshot.tasks
    : snapshot.features.map((feature) => ({
        id: feature.id,
        title: feature.title,
        status: feature.status,
        state: stateFromFeature(feature),
        parent: "features",
        claimed_by: null,
      }));
  const features = taskRows.map((task) => toFeatureRow(task));
  const previews = taskRows.map((task) => toTaskPreview(task));
  const milestones = buildMilestones(snapshot);
  const events = buildEvents(snapshot, now);
  const checks = buildChecks(snapshot);
  const configInspector = buildConfigInspector(snapshot, checks);
  const agentGrid = buildAgentGrid(snapshot);
  const dispatchQueue = buildDispatchQueue(taskRows);
  const taskBoard = buildTaskBoard(taskRows);
  const timelineMilestones = buildTimelineMilestones(milestones, features);
  const principleEffectiveness = buildPrincipleRows(snapshot);

  return {
    mode: "mission",
    missionId: "maestro-current",
    missionTitle: `Maestro Mission Control`,
    missionStatus: "executing",
    effectiveStatus: snapshot.summary.blocked > 0 ? "validating" : "executing",
    elapsedMs: 0,
    featureProgress: {
      done: snapshot.summary.done,
      total: snapshot.summary.workable_cards,
      active: snapshot.summary.active,
    },
    statusProgress: {
      completed: snapshot.summary.done,
      total: snapshot.summary.workable_cards,
      inFlight: snapshot.summary.active,
      blocked: snapshot.summary.blocked,
      queued: snapshot.summary.ready,
      completionPct: pct(snapshot.summary.done, snapshot.summary.workable_cards),
    },
    tokenCounters: null,
    missionOverview: {
      missionLabel: snapshot.repo.branch
        ? `${snapshot.repo.branch}`
        : "current checkout",
      statusLabel: snapshot.summary.blocked > 0 ? "validating" : "executing",
      activeCount: snapshot.summary.active,
      doneCount: snapshot.summary.done,
      totalCount: snapshot.summary.workable_cards,
      blockedCount: snapshot.summary.blocked,
      currentMilestone: "Current Maestro cards",
      gateLabel: snapshot.config.read_only ? "read-only" : null,
      agentSummary: agentGrid.map((agent) => ({
        agent: agent.agentType,
        count: agent.featureCount,
      })),
      dependencyMap: [],
    },
    activeFeature: previews.find((preview) => preview.status === "in-progress") ?? previews[0] ?? null,
    features,
    taskPreviews: previews,
    session: {
      branch: snapshot.repo.branch ?? "(detached)",
      workingTreeClean: !snapshot.repo.dirty,
      diffStat: `${snapshot.repo.code_other_dirty} code/other, ${snapshot.repo.maestro_dirty} maestro`,
      changedFiles: snapshot.repo.dirty ? ["working tree has uncommitted changes"] : [],
      fileChanges: snapshot.repo.dirty
        ? [{ path: "working tree has uncommitted changes", kind: "modified" }]
        : [],
    },
    configSummary: {
      configSource: "project",
      gitAvailable: snapshot.repo.branch !== null,
      checks,
      missionDirectory: `${snapshot.repo.root}/.maestro`,
      backgroundMode: "solid",
    },
    configInspector,
    progressLog: events,
    milestones,
    gateBlocked: snapshot.summary.blocked > 0,
    gateLabel: snapshot.summary.blocked > 0 ? `${snapshot.summary.blocked} blocked` : "clear",
    canPause: false,
    canResume: false,
    memory: { _retired: true },
    agentGrid,
    dispatchQueue,
    eventStream: events.map((event) => ({
      timestamp: event.timestamp,
      relativeMs: event.relativeMs,
      kind: event.kind,
      title: event.title,
      detail: event.detail,
    })),
    taskBoard,
    timelineMilestones,
    replyInbox: [],
    principleEffectiveness,
    home: {
      headline: "Current Maestro workspace",
      summary: `${snapshot.summary.workable_cards} cards, ${snapshot.summary.ready} ready, ${snapshot.summary.active} active`,
      locationLabel: snapshot.repo.root,
      checks,
      actions: [
        {
          label: "ready",
          command: "maestro card ready",
          detail: "Inspect claimable cards",
        },
        {
          label: "watch",
          command: "maestro watch snapshot",
          detail: "Compare the Rust-native live board",
        },
      ],
    },
  };
}

function stateFromFeature(feature: RustFeatureSnapshot): string {
  if (feature.active > 0 || feature.status === "in_progress") return "active";
  if (feature.needs_verification > 0) return "needs_verification";
  if (feature.blocked > 0) return "blocked";
  if (feature.ready > 0) return "ready";
  if (feature.done > 0 || ["closed", "shipped", "verified"].includes(feature.status)) return "done";
  return feature.status;
}

function toFeatureRow(task: RustTaskSnapshot): MissionControlFeatureRow {
  return {
    id: task.id,
    title: task.title,
    status: mapFeatureStatus(task.state || task.status),
    milestoneId: task.parent ?? "cards",
    agentType: task.claimed_by ?? "unclaimed",
    hasReport: ["done", "verified", "closed", "shipped"].includes(task.status),
    blockedByIds: task.state === "blocked" ? ["blocked"] : [],
    blockedByLabel: task.state === "blocked" ? "blocked" : undefined,
  };
}

function toTaskPreview(task: RustTaskSnapshot): TaskPreviewPane {
  const status = mapFeatureStatus(task.state || task.status);
  return {
    id: task.id,
    title: task.title,
    status,
    milestoneId: task.parent ?? "cards",
    milestoneTitle: task.parent ?? "Current Maestro cards",
    agentType: task.claimed_by ?? "unclaimed",
    description: `Current Maestro card sourced from Rust snapshot. Raw status: ${task.status}; state: ${task.state}.`,
    preconditions: undefined,
    expectedBehavior: undefined,
    verificationSteps: [],
    dependsOn: [],
    blockedBy: status === "blocked"
      ? [{ id: "blocked", title: "Blocked in current card store", status: "blocked" }]
      : [],
    unblocks: [],
    fulfills: [],
    validTransitions: [],
  };
}

function buildMilestones(snapshot: RustMissionControlSnapshot) {
  return [
    {
      id: "cards",
      title: "Current Maestro cards",
      status: snapshot.summary.active > 0 ? "executing" : "validating",
      order: 1,
      kind: "work",
      profile: "implementation",
    },
    {
      id: "proof",
      title: "Proof / Verify",
      status: snapshot.proof.proof_failed > 0 ? "failed" : "validating",
      order: 2,
      kind: "gate",
      profile: "validation",
    },
  ] as const;
}

function buildEvents(snapshot: RustMissionControlSnapshot, now: number): MissionControlEvent[] {
  const sessionEvents = snapshot.sessions.map((session, index) => {
    const eventTime = new Date(now - session.age_minutes * 60_000).toISOString();
    return {
      timestamp: eventTime,
      relativeMs: -session.age_minutes * 60_000,
      kind: "mission" as const,
      title: `${session.agent_runtime ?? "agent"} ${session.presence}`,
      detail: `${session.bound_card ?? "unbound"} · ${session.last_action}`,
    };
  });
  const proofEvents: MissionControlEvent[] = [
    {
      timestamp: new Date(now).toISOString(),
      relativeMs: 0,
      kind: "checkpoint",
      title: "Proof buckets",
      detail: `${snapshot.proof.proof_missing} missing, ${snapshot.proof.proof_stale} stale, ${snapshot.proof.proof_failed} failed`,
    },
  ];
  return [...sessionEvents, ...proofEvents].slice(0, 80);
}

function buildChecks(snapshot: RustMissionControlSnapshot): DoctorCheck[] {
  return [
    {
      id: "snapshot",
      label: "Rust snapshot",
      status: snapshot.schema === "maestro.mission_control.snapshot.v1" ? "ok" : "fail",
      message: snapshot.config.source,
    },
    {
      id: "working-tree",
      label: "Working tree",
      status: snapshot.repo.dirty ? "warn" : "ok",
      message: snapshot.repo.dirty
        ? `${snapshot.repo.code_other_dirty + snapshot.repo.maestro_dirty} uncommitted paths`
        : "clean",
    },
    {
      id: "proof",
      label: "Proof",
      status: snapshot.proof.proof_failed > 0 ? "fail" : snapshot.proof.proof_missing > 0 ? "warn" : "ok",
      message: `${snapshot.proof.proof_missing} missing, ${snapshot.proof.proof_failed} failed`,
    },
  ];
}

function buildConfigInspector(
  snapshot: RustMissionControlSnapshot,
  checks: readonly DoctorCheck[],
): MissionControlConfigInspector {
  const rowsByTab = Object.fromEntries(
    CONFIG_TABS.map((tab) => [tab, [] as MissionControlConfigRow[]]),
  ) as Record<MissionControlConfigTab, MissionControlConfigRow[]>;
  rowsByTab.overview = [
    readonlyRow("repo.root", "Repo", snapshot.repo.root, "Current workspace root"),
    readonlyRow("repo.branch", "Branch", snapshot.repo.branch ?? "(detached)", "Current git branch"),
    readonlyRow("config.source", "Source", snapshot.config.source, "Rust snapshot provider"),
  ];
  rowsByTab.effective = [
    readonlyRow("renderer", "Renderer", "TypeScript/OpenTUI", "Restored frontend renderer"),
    readonlyRow("readOnly", "Read only", String(snapshot.config.read_only), "Preview paths do not mutate card artifacts"),
  ];
  rowsByTab.doctor = checks.map((check) =>
    readonlyRow(
      check.id ?? check.label ?? "check",
      check.label ?? check.id ?? "Check",
      check.message,
      check.detail ?? check.status,
      check.status === "ok" ? "default" : "none",
    )
  );
  rowsByTab.plan = [
    readonlyRow("plan.next", "Next customization", "Adapt labels and write actions after restore", "The old UI shell is live first"),
  ];
  rowsByTab.memory = [
    readonlyRow("memory.retired", "Memory", "retired", "Old memory graph panels are retained as UI shells"),
  ];

  return {
    tabs: CONFIG_TABS,
    rowsByTab,
    hasProjectConfig: true,
    hasGlobalConfig: false,
    projectPath: `${snapshot.repo.root}/.maestro`,
    globalPath: "",
    errors: [],
  };
}

function readonlyRow(
  keyPath: string,
  label: string,
  value: string,
  summary: string,
  source: "project" | "global" | "default" | "mixed" | "none" = "project",
): MissionControlConfigRow {
  return {
    keyPath,
    label,
    section: "Snapshot",
    valueText: value,
    displayValueText: value,
    source,
    sourceBadge: source === "project" ? "P" : source === "global" ? "G" : source === "default" ? "D" : "",
    editKind: "readonly",
    editKindLabel: "read-only",
    description: summary,
    summary,
    impactText: "Inspection only in the restored TUI sidecar.",
    effectiveValueText: value,
    effectiveDisplayValueText: value,
  };
}

function buildAgentGrid(snapshot: RustMissionControlSnapshot): AgentGridRow[] {
  const byAgent = new Map<string, AgentGridRow>();
  for (const session of snapshot.sessions) {
    const agentType = session.agent_runtime ?? "agent";
    const previous = byAgent.get(agentType);
    byAgent.set(agentType, {
      agentType,
      status: session.presence === "working" ? "active" : "stale",
      activeFeatureId: session.bound_card ?? undefined,
      activeFeatureTitle: session.bound_card ?? undefined,
      lastActivityAt: new Date(Date.now() - session.age_minutes * 60_000).toISOString(),
      featureCount: (previous?.featureCount ?? 0) + 1,
      completedCount: previous?.completedCount ?? 0,
    });
  }
  return [...byAgent.values()];
}

function buildDispatchQueue(tasks: readonly RustTaskSnapshot[]): DispatchQueueItem[] {
  return tasks
    .filter((task) => task.state === "ready")
    .slice(0, 80)
    .map((task, index) => ({
      featureId: task.id,
      featureTitle: task.title,
      milestoneId: task.parent ?? "cards",
      milestoneTitle: task.parent ?? "Current Maestro cards",
      milestoneOrder: index + 1,
      agentType: task.claimed_by ?? "unclaimed",
    }));
}

function buildTaskBoard(tasks: readonly RustTaskSnapshot[]): TaskBoardSnapshot {
  const columns: Record<TaskStatus, TaskBoardItem[]> = {
    pending: [],
    in_progress: [],
    completed: [],
  };
  for (const task of tasks) {
    const column = mapTaskStatus(task.state);
    columns[column].push({
      id: task.id,
      title: task.title,
      status: column,
      priority: task.state === "blocked" ? "high" : "normal",
      assignee: task.claimed_by ?? undefined,
      labels: [task.status, task.state].filter(Boolean),
      blockedByCount: task.state === "blocked" ? 1 : 0,
      evidenceCount: ["done", "needs_verification"].includes(task.state) ? 1 : 0,
      recentEvidence: [],
    });
  }
  return {
    columns,
    totalCount: tasks.length,
  };
}

function buildTimelineMilestones(
  milestones: ReturnType<typeof buildMilestones>,
  features: readonly MissionControlFeatureRow[],
): TimelineMilestoneEntry[] {
  return milestones.map((milestone) => ({
    id: milestone.id,
    title: milestone.title,
    order: milestone.order,
    kind: milestone.kind,
    profile: milestone.profile,
    features: features.slice(0, 50).map((feature) => ({
      id: feature.id,
      title: feature.title,
      status: feature.status,
    })),
    progressPct: pct(features.filter((feature) => feature.status === "done").length, features.length),
  }));
}

function buildPrincipleRows(snapshot: RustMissionControlSnapshot): PrincipleEffectivenessRow[] {
  return [
    {
      id: "proof",
      name: "Proof health",
      mode: "advisory",
      helpful: snapshot.proof.proof_accepted + snapshot.proof.verified_or_done,
      unhelpful: snapshot.proof.proof_failed,
      pending: snapshot.proof.proof_missing + snapshot.proof.proof_stale,
      total: snapshot.summary.workable_cards,
      effectivenessPct: pct(
        snapshot.proof.proof_accepted + snapshot.proof.verified_or_done,
        snapshot.proof.proof_accepted + snapshot.proof.verified_or_done + snapshot.proof.proof_failed,
      ),
      lowSample: snapshot.summary.workable_cards < 3,
      recentKickbackExamples: [],
    },
  ];
}

function mapFeatureStatus(value: string): FeatureStatus {
  switch (value) {
    case "active":
    case "in_progress":
    case "in-progress":
      return "in-progress";
    case "needs_verification":
    case "review":
    case "verified":
      return "review";
    case "done":
    case "closed":
    case "shipped":
    case "completed":
      return "done";
    case "blocked":
      return "blocked";
    case "ready":
    case "draft":
    case "proposed":
    default:
      return "pending";
  }
}

function mapTaskStatus(value: string): TaskStatus {
  if (value === "active" || value === "in_progress") return "in_progress";
  if (value === "done" || value === "closed" || value === "verified") return "completed";
  return "pending";
}

function pct(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.round((done / total) * 100);
}
