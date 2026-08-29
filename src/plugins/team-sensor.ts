import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Store, resolveStoreLocation } from "../kernel/store.ts";

const evidenceLimit = 4_096;
const excerptLimit = 8_192;
const agentTailLimit = 16_384;
const defaultSilenceMs = 5 * 60 * 1_000;
const defaultPollMs = 15_000;
const startupWaitMs = 60_000;
const maxAgents = 32;
const maxPacketsPerCycle = 16;

export interface SensorAgentEvidence {
  name: string;
  status: string;
  text: string;
}

export interface SensorDispatchEvidence {
  actor: string;
  handbackFiled: boolean;
  id: string;
  lastProgressAt: string;
  stopCondition: string;
  workId: string;
}

export interface SensorWorkEvidence {
  id: string;
  state: string;
  updatedAt: string;
}

export interface SensorDetectionInput {
  agents: SensorAgentEvidence[];
  dispatches: SensorDispatchEvidence[];
  now: Date;
  silenceMs: number;
  teamId: string;
  teamVerdict: string;
  work: SensorWorkEvidence[];
}

export interface SensorCandidate {
  actor: string;
  dedupeKey: string;
  evidence: string;
  excerpt: string;
  occurrences: number;
  ruleId:
    | "semantic.failure-third"
    | "semantic.role-boundary"
    | "semantic.self-correction"
    | "semantic.status-contradiction"
    | "semantic.stop-silence";
  stopRef?: string;
  workRef?: string;
}

export interface TeamSensorCycleConfig {
  env?: Record<string, string | undefined>;
  generation: number;
  now?: Date;
  observerName: string;
  repoPath: string;
  silenceMs?: number;
  teamId: string;
  workspaceId: string;
}

export interface TeamSensorCycleResult {
  candidates: SensorCandidate[];
  deduped: number;
  emitted: number;
  stage: "ACTIVE" | "STARTING" | "STOPPING";
}

interface LocalBindingRow {
  binding_id: string;
  generation: number;
  project_root: string;
  project_store_path: string;
  room_identity: string;
  room_store_path: string;
  team_id: string;
  token: string;
}

interface RoomBindingRow extends LocalBindingRow {
  status: string;
}

interface TeamRow {
  generation: number;
  plan_json: string;
  repo_path: string;
  stage: string;
  verdict: string;
  workspace_label: string;
}

interface AgentRecord {
  agent_status?: string;
  name?: string;
  workspace_id?: string;
}

interface WorkspaceRecord {
  label?: string;
  workspace_id?: string;
}

interface SensorAuthority {
  dispatches: SensorDispatchEvidence[];
  roomRoot: string;
  stage: "ACTIVE" | "STARTING" | "STOPPING";
  teamVerdict: string;
  work: SensorWorkEvidence[];
  workspaceLabel: string;
}

export class TeamSensorAuthorityError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly pending = false,
  ) {
    super(message);
  }
}

function json<T>(value: string): T {
  return JSON.parse(value) as T;
}

function tableExists(store: Store, name: string): boolean {
  return store.database
    .query<{ name: string }, [string]>(
      "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get(name) !== null;
}

function canonicalPath(path: string): string {
  try {
    return realpathSync(path);
  } catch {
    return resolve(path);
  }
}

function boundedEvidence(value: string): string {
  return value.slice(0, evidenceLimit);
}

function boundedExcerpt(value: string): string {
  return value.slice(-excerptLimit);
}

function normalizedLine(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function candidate(
  teamId: string,
  input: Omit<SensorCandidate, "dedupeKey">,
): SensorCandidate {
  const bounded = {
    ...input,
    evidence: boundedEvidence(input.evidence),
    excerpt: boundedExcerpt(input.excerpt),
  };
  return {
    ...bounded,
    dedupeKey: createHash("sha256")
      .update(JSON.stringify({ teamId, ...bounded }))
      .digest("hex"),
  };
}

function firstLine(lines: string[], pattern: RegExp): string | null {
  return lines.find((line) => pattern.test(line)) ?? null;
}

function latestIso(...values: Array<string | null | undefined>): string {
  return values
    .filter((value): value is string => Boolean(value && Number.isFinite(Date.parse(value))))
    .sort((left, right) => Date.parse(right) - Date.parse(left))[0] ?? new Date(0).toISOString();
}

export function detectSensorCandidates(input: SensorDetectionInput): SensorCandidate[] {
  const candidates: SensorCandidate[] = [];
  const workById = new Map(input.work.map((work) => [work.id, work]));
  for (const agent of [...input.agents].sort((left, right) => left.name.localeCompare(right.name))) {
    const excerpt = boundedExcerpt(agent.text);
    const lines = excerpt.split("\n").map((line) => line.trim()).filter(Boolean);

    const failures = new Map<string, { count: number; line: string }>();
    for (const line of lines) {
      if (!/\b(error|failed|failure|lease_held|gate_blocked)\b/i.test(line)) continue;
      const key = normalizedLine(line);
      const current = failures.get(key) ?? { count: 0, line };
      failures.set(key, { count: current.count + 1, line: current.line });
    }
    const repeatedFailure = [...failures.values()].find((failure) => failure.count >= 3);
    if (repeatedFailure) {
      candidates.push(candidate(input.teamId, {
        actor: agent.name,
        evidence: `${repeatedFailure.line}\noccurrences=${repeatedFailure.count}`,
        excerpt,
        occurrences: repeatedFailure.count,
        ruleId: "semantic.failure-third",
      }));
    }

    const contradictions: Array<{ line: string; workRef?: string }> = [];
    if (input.teamVerdict !== "OPERABLE") {
      const line = lines.find((candidate) =>
        /\b(?:team\s+(?:is\s+)?)?(?:operable|ready)\b/i.test(candidate) &&
        !/\bnot(?:\s+yet)?\s+(?:operable|ready)\b/i.test(candidate)
      ) ?? null;
      if (line) contradictions.push({ line });
    }
    for (const work of input.work) {
      if (work.state === "done") continue;
      const id = escapeRegExp(work.id);
      const line = firstLine(
        lines,
        new RegExp(`\\b${id}\\b.{0,80}\\b(?:done|complete|completed)\\b`, "i"),
      );
      if (line) contradictions.push({ line, workRef: work.id });
    }
    if (contradictions.length > 0) {
      const selected = contradictions[0] as { line: string; workRef?: string };
      candidates.push(candidate(input.teamId, {
        actor: agent.name,
        evidence: `${selected.line}\nactualTeamVerdict=${input.teamVerdict}` +
          (selected.workRef ? `\nactualWorkState=${workById.get(selected.workRef)?.state ?? "unknown"}` : ""),
        excerpt,
        occurrences: 1,
        ruleId: "semantic.status-contradiction",
        ...(selected.workRef ? { workRef: selected.workRef } : {}),
      }));
    }

    const roleBoundary = firstLine(
      lines,
      /\b(not my role|outside my role|not mine to authorize|outside my authority)\b/i,
    );
    if (roleBoundary) {
      candidates.push(candidate(input.teamId, {
        actor: agent.name,
        evidence: roleBoundary,
        excerpt,
        occurrences: 1,
        ruleId: "semantic.role-boundary",
      }));
    }

    const selfCorrections = lines.filter((line) =>
      /\b(i was wrong|let me reconsider|actually, no|that was wrong|i misread|on second thought)\b/i.test(line)
    );
    if (selfCorrections.length >= 2) {
      candidates.push(candidate(input.teamId, {
        actor: agent.name,
        evidence: selfCorrections.join("\n"),
        excerpt,
        occurrences: selfCorrections.length,
        ruleId: "semantic.self-correction",
      }));
    }
  }

  for (const dispatch of [...input.dispatches].sort((left, right) => left.id.localeCompare(right.id))) {
    if (dispatch.handbackFiled || !dispatch.stopCondition.trim()) continue;
    const lastProgress = Date.parse(dispatch.lastProgressAt);
    if (!Number.isFinite(lastProgress) || input.now.getTime() - lastProgress < input.silenceMs) continue;
    candidates.push(candidate(input.teamId, {
      actor: dispatch.actor,
      evidence: [
        `dispatch=${dispatch.id}`,
        `lastProgressAt=${dispatch.lastProgressAt}`,
        `stop=${dispatch.stopCondition}`,
        "handback=missing",
      ].join("\n"),
      excerpt: `No handback was recorded after the explicit stop condition: ${dispatch.stopCondition}`,
      occurrences: 1,
      ruleId: "semantic.stop-silence",
      stopRef: dispatch.stopCondition,
      workRef: dispatch.workId,
    }));
  }
  return candidates;
}

function loadAuthority(config: TeamSensorCycleConfig): SensorAuthority {
  const repoPath = resolve(config.repoPath);
  const projectLocation = resolveStoreLocation(repoPath);
  const projectStore = new Store(projectLocation.path, { readonly: true });
  let local: LocalBindingRow | null = null;
  let work: SensorWorkEvidence[] = [];
  let dispatches: SensorDispatchEvidence[] = [];
  try {
    if (!tableExists(projectStore, "team_local_bindings")) {
      throw new TeamSensorAuthorityError(
        "BINDING_PENDING",
        `team ${config.teamId} has no local binding yet`,
        true,
      );
    }
    local = projectStore.database
      .query<LocalBindingRow, [string]>(
        "SELECT * FROM team_local_bindings WHERE team_id = ?",
      )
      .get(config.teamId) ?? null;
    if (!local) {
      throw new TeamSensorAuthorityError(
        "BINDING_PENDING",
        `team ${config.teamId} has no local binding yet`,
        true,
      );
    }
    if (local.generation !== config.generation) {
      throw new TeamSensorAuthorityError(
        "STALE_GENERATION",
        `sensor generation ${config.generation} does not match local binding generation ${local.generation}`,
      );
    }
    if (
      canonicalPath(local.project_root) !== canonicalPath(repoPath) ||
      canonicalPath(local.project_store_path) !== canonicalPath(projectLocation.path)
    ) {
      throw new TeamSensorAuthorityError(
        "BINDING_MISMATCH",
        "local binding does not match the sensor repository",
      );
    }
    if (tableExists(projectStore, "work")) {
      work = projectStore.database
        .query<{ id: string; state: string; updated_at: string }, []>(
          "SELECT id, state, updated_at FROM work ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)",
        )
        .all()
        .map((row) => ({ id: row.id, state: row.state, updatedAt: row.updated_at }));
    }
    if (tableExists(projectStore, "dispatches") && tableExists(projectStore, "handbacks")) {
      const progressByWork = new Map(work.map((row) => [row.id, row.updatedAt] as const));
      if (tableExists(projectStore, "work_notes")) {
        for (const row of projectStore.database
          .query<{ last_progress: string; work_id: string }, []>(
            "SELECT work_id, MAX(created_at) AS last_progress FROM work_notes GROUP BY work_id",
          )
          .all()) {
          progressByWork.set(row.work_id, latestIso(progressByWork.get(row.work_id), row.last_progress));
        }
      }
      const eventProgress = new Map<string, string>();
      if (tableExists(projectStore, "event_log")) {
        for (const row of projectStore.database
          .query<{ entity_id: string; last_progress: string }, []>(
            `SELECT entity_id, MAX(created_at) AS last_progress
             FROM event_log
             WHERE entity_id IS NOT NULL AND entity_type IN ('work', 'dispatch')
             GROUP BY entity_id`,
          )
          .all()) {
          eventProgress.set(row.entity_id, row.last_progress);
        }
      }
      const heldByWork = new Map(
        tableExists(projectStore, "work")
          ? projectStore.database
            .query<{ held_by: string | null; id: string }, []>("SELECT id, held_by FROM work")
            .all()
            .map((row) => [row.id, row.held_by] as const)
          : [],
      );
      dispatches = projectStore.database
        .query<{
          created_at: string;
          handback_id: string | null;
          id: string;
          pane: string | null;
          stop_condition: string;
          target_session: string | null;
          updated_at: string;
          work_id: string;
        }, []>(
          `SELECT d.id, d.work_id, d.stop_condition, d.pane, d.target_session,
                  d.created_at, d.updated_at, h.id AS handback_id
           FROM dispatches d
           LEFT JOIN handbacks h ON h.dispatch_id = d.id
           WHERE d.cancelled_at IS NULL
           ORDER BY CAST(SUBSTR(d.id, 2) AS INTEGER)`,
        )
        .all()
        .map((row) => ({
          actor: row.pane?.match(/^(?:lead|peer|advisor)-/) ? row.pane :
            row.target_session ?? heldByWork.get(row.work_id) ?? `dispatch-${row.id}`,
          handbackFiled: row.handback_id !== null,
          id: row.id,
          lastProgressAt: latestIso(
            row.updated_at,
            row.created_at,
            progressByWork.get(row.work_id),
            eventProgress.get(row.id),
            eventProgress.get(row.work_id),
          ),
          stopCondition: row.stop_condition,
          workId: row.work_id,
        }));
    }
  } finally {
    projectStore.close();
  }

  const roomStore = new Store(local.room_store_path, { readonly: true });
  try {
    if (!tableExists(roomStore, "team_project_bindings") || !tableExists(roomStore, "team_lifecycle")) {
      throw new TeamSensorAuthorityError(
        "BINDING_PENDING",
        `Room ledger for ${config.teamId} is not ready`,
        true,
      );
    }
    const authoritative = roomStore.database
      .query<RoomBindingRow, [string]>(
        "SELECT * FROM team_project_bindings WHERE binding_id = ?",
      )
      .get(local.binding_id);
    if (
      !authoritative ||
      authoritative.status !== "ACTIVE" ||
      authoritative.token !== local.token ||
      authoritative.room_identity !== local.room_identity ||
      authoritative.team_id !== config.teamId ||
      authoritative.generation !== config.generation ||
      canonicalPath(authoritative.project_root) !== canonicalPath(local.project_root) ||
      canonicalPath(authoritative.project_store_path) !== canonicalPath(local.project_store_path)
    ) {
      throw new TeamSensorAuthorityError(
        "BINDING_MISMATCH",
        "local sensor binding does not match the authoritative Room binding",
      );
    }
    const identity = tableExists(roomStore, "team_store_identity")
      ? roomStore.database
        .query<{ identity: string }, []>(
          "SELECT identity FROM team_store_identity WHERE singleton = 1",
        )
        .get()?.identity
      : null;
    if (identity !== local.room_identity) {
      throw new TeamSensorAuthorityError(
        "BINDING_MISMATCH",
        "Room store identity does not match the local sensor binding",
      );
    }
    const team = roomStore.database
      .query<TeamRow, [string]>("SELECT * FROM team_lifecycle WHERE team_id = ?")
      .get(config.teamId);
    if (!team) {
      throw new TeamSensorAuthorityError(
        "BINDING_PENDING",
        `Room lifecycle for ${config.teamId} is not ready`,
        true,
      );
    }
    if (team.generation !== config.generation) {
      throw new TeamSensorAuthorityError(
        "STALE_GENERATION",
        `sensor generation ${config.generation} does not match Room generation ${team.generation}`,
      );
    }
    if (canonicalPath(team.repo_path) !== canonicalPath(repoPath)) {
      throw new TeamSensorAuthorityError(
        "BINDING_MISMATCH",
        `Room lifecycle repository ${team.repo_path} does not match ${repoPath}`,
      );
    }
    if (team.stage !== "ACTIVE" && team.stage !== "STARTING" && team.stage !== "STOPPING") {
      throw new TeamSensorAuthorityError(
        "TEAM_CLOSED",
        `team ${config.teamId} is ${team.stage}`,
      );
    }
    const plan = json<{
      roles?: Array<{ agentName?: string; role?: string }>;
    }>(team.plan_json);
    const expectedObserver = plan.roles?.find((role) => role.role === "observer")?.agentName;
    if (expectedObserver !== config.observerName) {
      throw new TeamSensorAuthorityError(
        "OBSERVER_MISMATCH",
        `sensor observer ${config.observerName} does not match ${expectedObserver ?? "missing plan role"}`,
      );
    }
    return {
      dispatches,
      roomRoot: dirname(dirname(resolve(local.room_store_path))),
      stage: team.stage as SensorAuthority["stage"],
      teamVerdict: team.verdict,
      work,
      workspaceLabel: team.workspace_label,
    };
  } finally {
    roomStore.close();
  }
}

async function runJsonCommand(
  args: string[],
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<Record<string, unknown>> {
  const child = Bun.spawn(args, { cwd, env: { ...process.env, ...env }, stderr: "pipe", stdout: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`${args.slice(0, 3).join(" ")} failed (${exitCode}): ${stderr.trim()}`);
  }
  try {
    return JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    throw new Error(`${args.slice(0, 3).join(" ")} returned invalid JSON: ${stdout.slice(0, 500)}`);
  }
}

async function runTextCommand(
  args: string[],
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<string> {
  const child = Bun.spawn(args, { cwd, env: { ...process.env, ...env }, stderr: "pipe", stdout: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`${args.slice(0, 3).join(" ")} failed (${exitCode}): ${stderr.trim()}`);
  }
  return stdout;
}

function resultRecord(value: Record<string, unknown>): Record<string, unknown> {
  const result = value.result;
  return result && typeof result === "object" ? result as Record<string, unknown> : {};
}

function recordArray<T>(value: Record<string, unknown>, name: string): T[] {
  const nested = resultRecord(value)[name];
  return Array.isArray(nested) ? nested as T[] : [];
}

async function emitCandidate(
  config: TeamSensorCycleConfig,
  roomRoot: string,
  candidate: SensorCandidate,
): Promise<boolean> {
  const operationId = `sensor-${candidate.dedupeKey.slice(0, 40)}`;
  const args = [
    process.execPath,
    resolve(import.meta.dir, "..", "..", "bin", "maestro.ts"),
    "team",
    "review",
    "trigger",
    config.teamId,
    "--operation",
    operationId,
    "--rule",
    candidate.ruleId,
    "--actor",
    candidate.actor,
    "--evidence",
    candidate.evidence,
    "--excerpt",
    candidate.excerpt,
    "--occurrences",
    String(candidate.occurrences),
    "--json",
  ];
  if (candidate.workRef) args.push("--work", candidate.workRef);
  if (candidate.stopRef) args.push("--stop", candidate.stopRef);
  const response = await runJsonCommand(args, roomRoot, {
    ...config.env,
    MAESTRO_SESSION_ID: `team-sensor-${config.teamId}-g${config.generation}`,
    MAESTRO_SESSION_PID: String(process.pid),
  });
  const data = response.data;
  return Boolean(data && typeof data === "object" && (data as Record<string, unknown>).deduped === true);
}

export async function runTeamSensorCycle(
  config: TeamSensorCycleConfig,
  seenDedupeKeys = new Set<string>(),
): Promise<TeamSensorCycleResult> {
  const authority = loadAuthority(config);
  const env = config.env ?? {};
  const workspaceResponse = await runJsonCommand(["herdr", "workspace", "list"], config.repoPath, env);
  const workspace = recordArray<WorkspaceRecord>(workspaceResponse, "workspaces").filter(
    (candidate) => candidate.workspace_id === config.workspaceId && candidate.label === authority.workspaceLabel,
  );
  if (workspace.length !== 1) {
    throw new TeamSensorAuthorityError(
      "WORKSPACE_MISMATCH",
      `sensor workspace ${config.workspaceId} does not match ${authority.workspaceLabel}`,
    );
  }
  const agentResponse = await runJsonCommand(["herdr", "agent", "list"], config.repoPath, env);
  const workspaceAgents = recordArray<AgentRecord>(agentResponse, "agents").filter(
    (agent) => agent.workspace_id === config.workspaceId,
  );
  const observers = workspaceAgents.filter((agent) => agent.name === config.observerName);
  if (observers.length !== 1) {
    throw new TeamSensorAuthorityError(
      "OBSERVER_UNREACHABLE",
      `expected one ${config.observerName} in ${config.workspaceId}; found ${observers.length}`,
    );
  }
  if (
    !observers[0]?.agent_status ||
    ["stopped", "exited", "error"].includes(observers[0].agent_status)
  ) {
    throw new TeamSensorAuthorityError(
      "OBSERVER_UNREACHABLE",
      `${config.observerName} is ${observers[0]?.agent_status ?? "unknown"}`,
    );
  }
  const monitored = workspaceAgents.filter(
    (agent): agent is AgentRecord & { name: string } =>
      typeof agent.name === "string" && agent.name !== config.observerName,
  );
  if (monitored.length > maxAgents) {
    throw new TeamSensorAuthorityError(
      "AGENT_LIMIT",
      `workspace ${config.workspaceId} has ${monitored.length} monitored agents; limit is ${maxAgents}`,
    );
  }
  if (authority.stage !== "ACTIVE") {
    return { candidates: [], deduped: 0, emitted: 0, stage: authority.stage };
  }
  const agents: SensorAgentEvidence[] = [];
  for (const agent of monitored.sort((left, right) => left.name.localeCompare(right.name))) {
    const text = await runTextCommand(
      ["herdr", "agent", "read", agent.name, "--source", "recent-unwrapped", "--lines", "120"],
      config.repoPath,
      env,
    );
    agents.push({
      name: agent.name,
      status: agent.agent_status ?? "unknown",
      text: text.slice(-agentTailLimit),
    });
  }
  const detected = detectSensorCandidates({
    agents,
    dispatches: authority.dispatches,
    now: config.now ?? new Date(),
    silenceMs: config.silenceMs ?? defaultSilenceMs,
    teamId: config.teamId,
    teamVerdict: authority.teamVerdict,
    work: authority.work,
  });
  const candidates: SensorCandidate[] = [];
  let deduped = 0;
  let emitted = 0;
  for (const current of detected) {
    if (seenDedupeKeys.has(current.dedupeKey)) continue;
    if (emitted >= maxPacketsPerCycle) break;
    candidates.push(current);
    if (await emitCandidate(config, authority.roomRoot, current)) deduped += 1;
    else emitted += 1;
    seenDedupeKeys.add(current.dedupeKey);
  }
  return { candidates, deduped, emitted, stage: authority.stage };
}

export async function runTeamSensor(config: TeamSensorCycleConfig): Promise<number> {
  const deadline = Date.now() + startupWaitMs;
  let started = false;
  const seenDedupeKeys = new Set<string>();
  while (true) {
    try {
      const result = await runTeamSensorCycle(config, seenDedupeKeys);
      started = true;
      if (result.emitted > 0 || result.deduped > 0) {
        process.stdout.write(`${JSON.stringify({
          deduped: result.deduped,
          emitted: result.emitted,
          generation: config.generation,
          stage: result.stage,
          teamId: config.teamId,
        })}\n`);
      }
    } catch (error) {
      if (
        error instanceof TeamSensorAuthorityError &&
        error.pending &&
        !started &&
        Date.now() < deadline
      ) {
        await Bun.sleep(250);
        continue;
      }
      process.stderr.write(
        `maestro-team-sensor: ${error instanceof TeamSensorAuthorityError ? `${error.code}: ` : ""}` +
          `${error instanceof Error ? error.message : String(error)}\n`,
      );
      return 78;
    }
    await Bun.sleep(defaultPollMs);
  }
}
