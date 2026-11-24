import { existsSync } from "node:fs";
import { appendFile, mkdir, readFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError, type CliResult } from "../../kernel/cli.ts";
import { Store, resolveStoreLocation, tableExists } from "../../kernel/store.ts";
import {
  HerdrClient,
  SlpRuntimeError,
  type HerdrAgent,
  type HerdrEvent,
  type HerdrEventStream,
  type HerdrSubscription,
} from "./herdr-client.ts";
import { resolveHomeDirectory } from "../home.ts";
import { herdrPluginId } from "./plugin.ts";
import { acquireProcessLock, slpRuntimeDirectory } from "./process.ts";
import {
  activityMark,
  answersStall,
  compactEvidence,
  dedupeWindowMs,
  emptyState,
  heldActive,
  heldById,
  hubAgentName,
  latestEntry,
  latestRecord,
  openStore,
  type PendingLine,
  pushedRecently,
  pushWindowMs,
  readRuntimeState,
  recordEntry,
  recordEvidence,
  replayMaxMs,
  replayQuietMs,
  type RoleRow,
  roleRows,
  roleTarget,
  runningWaitMs,
  runtimeEnvironment,
  runtimeLockHolder,
  type RuntimeState,
  seatHasStar,
  silenceAnsweredMs,
  type SlpRuntimeConfig,
  stallCopyLine,
  type StallKind,
  stallLine,
  stopInProgress,
  teamCard,
  teamRow,
  type TeamRow,
  type WakeTarget,
  watchLines,
  type WorkRow,
  type WorkState,
  writeRuntimeState,
} from "./attention-rows.ts";

class AttentionRuntime {
  readonly directory: string;
  private state: RuntimeState = emptyState();
  private statuses = new Map<string, string>();
  private roles: RoleRow[] = [];
  // Panes where Herdr detected an agent before the seat's role row existed
  // (a Peer being opened by work add --to); their status events are wanted
  // so the row is found on the first event after the acknowledgement.
  private candidates = new Set<string>();
  private readonly client: HerdrClient;

  private logChain: Promise<void> = Promise.resolve();

  constructor(private readonly config: SlpRuntimeConfig) {
    this.directory = slpRuntimeDirectory(config.projectPath, config.teamId, config.generation);
    this.client = new HerdrClient(config.environment ?? process.env);
  }

  // runtime.log beside the lock: what the runtime saw and did, for the next
  // live defect (the pane itself is redrawn on every event).
  log(line: string): void {
    const stamped = `${new Date().toISOString()} ${line}\n`;
    process.stderr.write(stamped);
    this.logChain = this.logChain
      .then(() => appendFile(join(this.directory, "runtime.log"), stamped))
      .catch(() => undefined);
  }

  async load(): Promise<void> {
    await mkdir(this.directory, { recursive: true });
    this.state = (await readRuntimeState(this.directory)) ?? emptyState();
  }

  rolePanes(): string[] {
    return this.roles.map((role) => role.pane_id);
  }

  private async persist(): Promise<void> {
    await writeRuntimeState(this.directory, this.state);
  }

  private team(store: Store): TeamRow | null {
    return teamRow(store, this.config.teamId, this.config.generation);
  }

  // True only when a pane not yet subscribed appears: a role row landing on a
  // pane already watched as a candidate needs no new subscription (each one
  // costs a replay).
  refreshRoles(store: Store): boolean {
    const next = roleRows(store, this.config.teamId, this.config.generation);
    const watched = new Set([...this.roles.map((role) => role.pane_id), ...this.candidates]);
    this.roles = next;
    return next.some((role) => !watched.has(role.pane_id));
  }

  subscriptions(): HerdrSubscription[] {
    const panes = new Set([...this.roles.map((role) => role.pane_id), ...this.candidates]);
    return [
      ...[...panes].map((pane_id) => ({ pane_id, type: "pane.agent_status_changed" as const })),
      { type: "pane.created" },
      { type: "pane.agent_detected" },
      { type: "pane.exited" },
      { type: "pane.closed" },
    ];
  }

  async seedStatuses(): Promise<void> {
    let agents: HerdrAgent[] = [];
    try {
      agents = await this.client.agentList();
    } catch (error) {
      this.log(`agent.list failed while seeding statuses: ${error instanceof Error ? error.message : String(error)}`);
      return;
    }
    for (const agent of agents) this.statuses.set(agent.pane_id, agent.agent_status);
  }

  // After a (re)subscribe the replay was discarded; the live state of every
  // role pane is evaluated once so a transition inside that window is not lost.
  async evaluateCurrent(): Promise<boolean> {
    await this.seedStatuses();
    let resubscribe = false;
    for (const role of this.roles) {
      const status = this.statuses.get(role.pane_id);
      if (status !== "idle" && status !== "done") continue;
      resubscribe = (await this.handle({
        data: { agent_status: status, pane_id: role.pane_id, workspace_id: role.workspace_id },
        event: "pane_agent_status_changed",
      }, true)) || resubscribe;
    }
    return resubscribe;
  }

  // The Hub Supervisor's pane is the store fact team start recorded; the
  // Herdr agent `supervisor` is the fallback, and neither means unreachable.
  private async hubTarget(team: TeamRow): Promise<WakeTarget> {
    if (team.supervisor_pane_id) return { target: team.supervisor_pane_id };
    let listing = "";
    try {
      if ((await this.client.agentList()).some((agent) => agent.name === hubAgentName)) return { target: hubAgentName };
    } catch (error) {
      listing = `; agent list failed: ${error instanceof Error ? error.message : String(error)}`;
    }
    return {
      reason: `no Hub Supervisor pane recorded for ${team.team_id} g${team.generation} and no Herdr agent named ${hubAgentName}${listing}`,
      target: null,
    };
  }

  private seatAbove(seat: RoleRow, team: TeamRow): Promise<WakeTarget> {
    if (seat.role === "lead") return this.hubTarget(team);
    return Promise.resolve(roleTarget(this.roles, "lead"));
  }

  private async deliver(
    wake: WakeTarget,
    subject: string,
    line: string,
    stall?: { holder: string; state: WorkState; workId: string },
  ): Promise<void> {
    if (wake.target === null) {
      this.log(`wake about ${subject} is unreachable and dropped: ${wake.reason}; the store remains the truth`);
      this.state.unreachable.push({ droppedAt: new Date().toISOString(), line, reason: wake.reason, subject });
      await this.persist();
      return;
    }
    const target = wake.target;
    const queue = this.state.pending[target] ?? [];
    queue.push({ line, queuedAt: new Date().toISOString(), subject, ...(stall ? { stall } : {}) });
    this.state.pending[target] = queue;
    await this.flush(target);
  }

  // Queue: a wake for a working target waits for its own idle; a failed
  // prompt stays queued (SPEC item 4).
  async flush(target: string): Promise<void> {
    let queue = this.state.pending[target] ?? [];
    if (queue.length === 0) return;
    const pane = this.roles.find((role) => role.name === target)?.pane_id;
    const status = pane ? this.statuses.get(pane) : undefined;
    if (status === "working") {
      await this.persist();
      return;
    }
    // w708 (d867): a queued line waits for its target's own idle, which can be
    // much later - a copy queued for a busy reviewing seat was first delivered
    // forty minutes on, about an item returned, accepted and committed since.
    // A stall speaks about the present, so it is checked against the store at
    // delivery and dropped rather than sent stale.
    if (queue.some((entry) => entry.stall)) {
      const store = openStore(this.config.projectPath, true);
      try {
        const live = queue.filter((entry) => {
          if (!entry.stall) return true;
          const held = heldById(store, entry.stall.workId);
          const holder = held ? held.owner ?? held.assigned_to : null;
          if (held && held.state === entry.stall.state && holder === entry.stall.holder) return true;
          this.log(
            `stale wake about ${entry.subject} dropped: ${entry.stall.workId} is ${held ? `${held.state} for ${holder}` : "gone"},` +
              ` not ${entry.stall.state} for ${entry.stall.holder} as when it was queued`,
          );
          return false;
        });
        if (live.length !== queue.length) {
          queue = live;
          this.state.pending[target] = queue;
          if (queue.length === 0) {
            delete this.state.pending[target];
            await this.persist();
            return;
          }
        }
      } finally {
        store.close();
      }
    }
    while (queue.length > 0) {
      const next = queue[0] as PendingLine;
      try {
        await this.client.agentPrompt(target, next.line);
        queue.shift();
        this.state.delivered[target] = Date.now();
        this.log(`woke ${target}: ${next.subject}`);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.log(`wake of ${target} about ${next.subject} stays queued: ${message}`);
        break;
      }
    }
    if (queue.length === 0) delete this.state.pending[target];
    await this.persist();
  }

  private deduplicated(paneId: string, status: string, now: number): boolean {
    const key = `${paneId}:${status}`;
    const last = this.state.recent[key];
    for (const [candidate, at] of Object.entries(this.state.recent)) {
      if (now - at > dedupeWindowMs * 12) delete this.state.recent[candidate];
    }
    this.state.recent[key] = now;
    return last !== undefined && now - last < dedupeWindowMs;
  }

  // d763 once-rule: one stall entry and one nudge per (item, kind) until that
  // item's latest entry changes; the runtime's own entry is the mark.
  private async stall(
    store: Store,
    team: TeamRow,
    seat: RoleRow,
    kind: StallKind,
    work: WorkRow,
    evidence: string,
  ): Promise<void> {
    const key = `${work.id}:${kind}`;
    const holder = work.owner ?? work.assigned_to;
    // w708 (d867): the mark is the holder's own latest record. An entry written by
    // another seat - a reviewer's correction on an item it does not hold - no
    // longer releases the rule and re-arms the alarm against the holder.
    const own = latestRecord(store, work.id, holder);
    const mark = own?.mark ?? "none";
    if (this.state.stalls[key] === mark) return;
    // A seat that recorded on this item a moment ago is working, not silent; the
    // same window Advisor F15 uses for a push, for the same reason.
    // l3 (w712): a holder that recorded after the stall entry has answered this
    // nudge. The once-rule read that record as a store change and re-armed, so the
    // next idle sample past pushWindowMs sent the same nudge and the same
    // copy again - the seat was nudged for complying. Its account is
    // the strongest evidence the store has about a quiet pane, so it quiets the
    // same nudge for silenceAnsweredMs measured from itself. d96/d97 are
    // untouched: this withholds a nudge, it gates nothing.
    const answered = own !== null && answersStall(store, work.id, kind, own);
    if (own && Date.now() - Date.parse(own.at) < (answered ? silenceAnsweredMs : pushWindowMs)) {
      // The mark is deliberately not consumed here: withholding is about how long
      // ago the seat wrote, not about having answered for this record. A seat that
      // records and then goes quiet for good is still nudged once, later.
      this.log(
        `stall:${kind} on ${work.id} for ${seat.name} withheld: ${own.label} by ${own.actor} ` +
          `${answered ? "answers the nudge already sent" : "just recorded"}`,
      );
      await this.persist();
      return;
    }
    const body = `${kind}: ${evidence}`;
    const entryId = recordEntry(store, team, work.id, body, `stall:${kind}`, "work.stall");
    this.state.stalls[key] = mark;
    this.log(`stall:${kind} on ${work.id} for ${seat.name}: entry ${entryId}`);
    const line = stallLine(work.id, kind, evidence);
    const about = { holder, state: work.state, workId: work.id };
    await this.deliver({ target: seat.name }, `${work.id} ${kind}`, line, about);
    const copy = await this.hubTarget(team);
    if (copy.target !== seat.name) {
      await this.deliver(copy, `${work.id} ${kind} copy`, stallCopyLine(work.id, kind, evidence, holder), about);
    }
  }

  // Returns true when the subscription must be rebuilt (a new pane to watch).
  // An evaluation replays the current state after a subscribe: stalls are
  // derived from it, the idle wake is not (it needs a turn end seen live).
  async handle(event: HerdrEvent, evaluation = false): Promise<boolean> {
    const store = openStore(this.config.projectPath);
    try {
      const team = this.team(store);
      if (!team || team.state !== "RUNNING") return false;
      // A Peer opened after the subscription (work add --to) has no role row
      // until it acknowledged; every event re-reads the rows, and a pane
      // created or detected in the team workspace is watched right away.
      let changed = this.refreshRoles(store);
      if (changed) this.log(`roles now ${this.roles.map((role) => `${role.name}@${role.pane_id}`).join(", ")}`);
      const createdPane = event.event === "pane_created" && event.data.pane && typeof event.data.pane === "object"
        ? event.data.pane as { pane_id?: string; workspace_id?: string }
        : null;
      const paneId = typeof event.data.pane_id === "string"
        ? event.data.pane_id
        : typeof createdPane?.pane_id === "string" ? createdPane.pane_id : null;
      if (!paneId) return changed;
      const workspaceId = typeof event.data.workspace_id === "string"
        ? event.data.workspace_id
        : typeof createdPane?.workspace_id === "string" ? createdPane.workspace_id : null;
      if (event.event === "pane_agent_detected" || event.event === "pane_created") {
        const inTeam = workspaceId === null || workspaceId === team.workspace_id;
        const known = this.roles.some((role) => role.pane_id === paneId) || this.candidates.has(paneId);
        if (inTeam && !known) {
          this.candidates.add(paneId);
          this.log(`watching new team pane ${paneId} (${event.event})`);
        }
        return changed || (inTeam && !known);
      }
      const seat = this.roles.find((role) => role.pane_id === paneId);
      if (seat) this.candidates.delete(paneId);
      if (!seat) return changed;
      const now = Date.now();
      if (event.event === "pane_exited" || event.event === "pane_closed") {
        if (stopInProgress(store, team.team_id, team.generation)) return changed;
        const loss = event.event === "pane_exited" ? "exited" : "closed";
        if (this.deduplicated(paneId, loss, now)) return changed;
        this.statuses.delete(paneId);
        const card = teamCard(store, team.team_id, team.generation);
        const evidence = `${loss}: ${seat.name} pane ${paneId} (${event.event}); store: ${seat.role} of ${team.team_id} g${team.generation}`;
        if (card) recordEntry(store, team, card.id, evidence, `pane:${loss}`, "pane.lost");
        await this.deliver(await this.hubTarget(team), `${seat.name} pane ${loss}`, `[attention] ${seat.name} pane ${loss}`);
        return changed;
      }
      if (event.event !== "pane_agent_status_changed") return changed;
      const status = typeof event.data.agent_status === "string" ? event.data.agent_status : "unknown";
      this.statuses.set(paneId, status);
      // An evaluation is state, not a transition; it leaves the dedupe window
      // to the live events.
      if (!evaluation && this.deduplicated(paneId, status, now)) {
        await this.persist();
        return changed;
      }
      if (status !== "idle" && status !== "done") {
        await this.persist();
        return changed;
      }
      // d834: the silence stall is for delivery items only; the Lead holding
      // the team card ACTIVE while it waits on its Peers is not silent.
      const card = teamCard(store, team.team_id, team.generation);
      const held = heldActive(store, team.team_id, team.generation, seat.name, card?.id ?? null);
      if (held) {
        const holder = held.owner ?? held.assigned_to;
        // d761 layer one, d868: a seat that declared --blocked is legitimately
        // waiting. That declaration is the holder's own, so it is read from the
        // holder's own latest entry. Reading the widest record here let a decision
        // by the holder - or a note any other seat wrote on the item - stand newer
        // than a --blocked that had never been withdrawn, and nudge a seat that had
        // done exactly what the protocol asks of it. The holder's own later entry
        // does still release it: that is the seat saying it is no longer waiting.
        if (latestEntry(store, held.id, holder)?.flag === "blocked") {
          await this.flush(seat.name);
          return changed;
        }
        const latest = latestRecord(store, held.id);
        // w708, field evidence from the other team: ACTIVE and "owned by <seat>"
        // were template literals restating heldActive's where clause, so the two
        // words a reader uses to sanity-check a nudge were the two that carried no
        // measurement - which is what let a late line read as a fresh one. They are
        // read from the row here.
        const evidence = `${seat.name} pane ${paneId} agent_status ${status} while ${held.id} ${held.state}` +
          ` owned by ${held.owner ?? held.assigned_to}` + recordEvidence(latest, ";") +
          compactEvidence(store, paneId);
        await this.stall(store, team, seat, "silence", held, evidence);
        await this.flush(seat.name);
        return changed;
      }
      await this.flush(seat.name);
      if (evaluation) return changed;
      if (seatHasStar(store, team, seat, this.roles)) return changed;
      if (pushedRecently(store, team.team_id, team.generation, seat.name, now)) return changed;
      const delivered = this.state.delivered[seat.name];
      if (delivered !== undefined && now - delivered < pushWindowMs) return changed;
      // d832: one idle wake per pane until the team's activity log advances.
      const mark = activityMark(store, team.team_id, team.generation);
      if (this.state.idleWakes[paneId] === mark) return changed;
      this.state.idleWakes[paneId] = mark;
      await this.deliver(await this.seatAbove(seat, team), `${seat.name} idle`, `[attention] ${seat.name} idle`);
      return changed;
    } finally {
      store.close();
    }
  }

  async render(): Promise<string> {
    const sections = [`SLP runtime ${this.config.teamId}:g${this.config.generation}`];
    for (const role of this.roles) {
      const status = this.statuses.get(role.pane_id) ?? "unknown";
      let output = "[unavailable]";
      try {
        output = (await this.client.agentRead(role.name, "recent_unwrapped", watchLines)).trimEnd() || "[no output]";
      } catch {}
      sections.push(`=== ${role.name} [${role.role}] ${status} ===\n${output}`);
    }
    const pending = Object.entries(this.state.pending).flatMap(([target, lines]) =>
      lines.map((line) => `  ${target}: ${line.line}`)
    );
    if (pending.length > 0) sections.push(`pending wakes:\n${pending.join("\n")}`);
    return `${sections.join("\n\n")}\n`;
  }
}

export async function runSlpRuntime(config: SlpRuntimeConfig): Promise<number> {
  const runtime = new AttentionRuntime(config);
  await runtime.load();
  const lockPath = join(runtime.directory, "runtime.lock");
  await acquireProcessLock(lockPath, "SLP runtime");
  let stopping = false;
  let stream: HerdrEventStream | null = null;
  const stop = () => {
    stopping = true;
    stream?.close();
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
  process.on("SIGHUP", stop);
  try {
    const deadline = Date.now() + runningWaitMs;
    while (true) {
      const store = openStore(config.projectPath, true);
      let team: TeamRow | null;
      try {
        team = teamRow(store, config.teamId, config.generation);
        if (team?.state === "RUNNING") runtime.refreshRoles(store);
      } finally {
        store.close();
      }
      if (team?.state === "RUNNING") break;
      if (team?.state === "STOPPED") return 0;
      if (Date.now() >= deadline) {
        process.stderr.write(`maestro slp runtime: no running ${config.teamId}:g${config.generation} in ${config.projectPath}\n`);
        return 1;
      }
      await Bun.sleep(100);
    }
    const client = new HerdrClient(config.environment ?? process.env);
    runtime.log(`runtime up for ${config.teamId}:g${config.generation} (pid ${process.pid})`);
    const redraw = async () => {
      process.stdout.write(`\u001b[2J\u001b[H${await runtime.render()}`);
    };
    while (!stopping) {
      let resubscribe = false;
      const subscriptions = runtime.subscriptions();
      stream = await client.subscribe(subscriptions);
      runtime.log(`subscribed: ${subscriptions.map((entry) => "pane_id" in entry ? `${entry.type}:${entry.pane_id}` : entry.type).join(" ")}`);
      const iterator = stream.events[Symbol.asyncIterator]();
      let pending: Promise<IteratorResult<HerdrEvent>> | null = null;
      let ended = false;
      try {
        // Herdr replays its event history to every new subscriber (about 190
        // events over 12 s on 2026-09-05); those are discarded until the
        // stream has been quiet for replayQuietMs, then the live state of
        // every role pane is evaluated once.
        let replayed = 0;
        const drainStarted = Date.now();
        while (Date.now() - drainStarted < replayMaxMs) {
          pending ??= iterator.next();
          const outcome = await Promise.race([pending, Bun.sleep(replayQuietMs).then(() => "quiet" as const)]);
          if (outcome === "quiet") break;
          pending = null;
          if (outcome.done) {
            ended = true;
            break;
          }
          replayed += 1;
        }
        runtime.log(`replay drained: ${replayed} events discarded`);
        if (!ended) {
          resubscribe = await runtime.evaluateCurrent();
          await redraw();
        }
        while (!ended && !resubscribe) {
          pending ??= iterator.next();
          const outcome = await pending;
          pending = null;
          if (outcome.done) {
            ended = true;
            break;
          }
          try {
            resubscribe = await runtime.handle(outcome.value);
          } catch (error) {
            runtime.log(`event ${outcome.value.event} failed: ${error instanceof Error ? error.message : String(error)}`);
          }
          await redraw();
        }
      } finally {
        stream.close();
      }
      if (stopping) return 0;
      if (!resubscribe) {
        runtime.log("Herdr closed the subscription; the plugin startup hook reopens this pane after a restart");
        return 1;
      }
      runtime.log("resubscribing for a new team pane");
    }
    return 0;
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
    process.off("SIGHUP", stop);
    const holder = Number((await readFile(lockPath, "utf8").catch(() => "")).trim());
    if (holder === process.pid) await rm(lockPath, { force: true });
  }
}

export function runtimeConfigFromEnvironment(
  environment: Record<string, string | undefined> = process.env,
  cwd = process.cwd(),
): SlpRuntimeConfig {
  const teamId = environment[runtimeEnvironment.team];
  const generation = Number(environment[runtimeEnvironment.generation]);
  if (!teamId || !/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(teamId)) {
    throw new CliError("INVALID_OPTION", `${runtimeEnvironment.team} must name a normalized team id`);
  }
  if (!Number.isInteger(generation) || generation < 1) {
    throw new CliError("INVALID_OPTION", `${runtimeEnvironment.generation} must be a positive integer`);
  }
  return { environment, generation, projectPath: resolve(cwd), teamId };
}

// Every project the Hub registry lists; the plugin hooks run from the plugin
// directory and find running generations through it.
async function registeredProjects(home = resolveHomeDirectory()): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter((line) => line.trim() !== "");
}

interface RunningGeneration {
  projectPath: string;
  roles: RoleRow[];
  team: TeamRow;
}

function runningGenerations(projectPath: string): RunningGeneration[] {
  const location = resolveStoreLocation(projectPath);
  if (!existsSync(location.path)) return [];
  const store = new Store(location.path, { readonly: true });
  try {
    if (!tableExists(store, "slp_local_teams") || !tableExists(store, "slp_local_roles")) return [];
    return store.database
      .query<TeamRow, []>(
        `SELECT team_id, generation, project_path, state, workspace_id
         FROM slp_local_teams WHERE state = 'RUNNING' ORDER BY team_id, generation`,
      )
      .all()
      .map((team) => ({
        projectPath: team.project_path,
        roles: roleRows(store, team.team_id, team.generation),
        team,
      }));
  } finally {
    store.close();
  }
}

// Item 3: the pane command is static; team and generation arrive as env
// (advisor F17), cwd is the project so the store resolves like any command.
export async function openRuntimePane(
  client: HerdrClient,
  team: Pick<TeamRow, "generation" | "project_path" | "team_id" | "workspace_id">,
  anchorPaneId: string | null,
): Promise<string> {
  const pane = await client.pluginPaneOpen({
    cwd: team.project_path,
    entrypoint: "runtime",
    env: {
      [runtimeEnvironment.generation]: String(team.generation),
      [runtimeEnvironment.team]: team.team_id,
    },
    placement: anchorPaneId ? "split" : "tab",
    plugin_id: herdrPluginId,
    ...(anchorPaneId ? { target_pane_id: anchorPaneId } : { workspace_id: team.workspace_id }),
  });
  if (!pane?.pane_id) {
    throw new SlpRuntimeError("runtime pane was not opened", ["plugin", "pane", "open"]);
  }
  return pane.pane_id;
}

export function recordRuntimePane(projectPath: string, teamId: string, generation: number, paneId: string): void {
  const store = openStore(projectPath);
  try {
    store.database
      .query(`UPDATE slp_local_teams SET runtime_pane_id = ? WHERE team_id = ? AND generation = ?`)
      .run(paneId, teamId, generation);
  } finally {
    store.close();
  }
}

function recordedSupervisorPane(projectPath: string, teamId: string, generation: number): string {
  const store = openStore(projectPath, true);
  try {
    return teamRow(store, teamId, generation)?.supervisor_pane_id ?? "";
  } finally {
    store.close();
  }
}

function recordedRuntimePane(projectPath: string, teamId: string, generation: number): string {
  const store = openStore(projectPath, true);
  try {
    if (!store.hasColumn("slp_local_teams", "runtime_pane_id")) return "";
    return store.database
      .query<{ runtime_pane_id: string }, [string, number]>(
        `SELECT runtime_pane_id FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
      )
      .get(teamId, generation)?.runtime_pane_id ?? "";
  } finally {
    store.close();
  }
}

// Item 6 (d96, d759): after a Herdr restart, every RUNNING generation whose
// role panes survived gets its runtime pane back; one whose panes are all
// gone is noted as lost on the team card and nothing is re-created.
export async function runSlpRestore(
  environment: Record<string, string | undefined> = process.env,
  home = resolveHomeDirectory({ environmentHome: environment.HOME }),
): Promise<CliResult> {
  const client = new HerdrClient(environment);
  const lines: string[] = [];
  const restored: Array<Record<string, unknown>> = [];
  let agents: HerdrAgent[] | null = null;
  for (const projectPath of await registeredProjects(home)) {
    let generations: RunningGeneration[];
    try {
      generations = runningGenerations(projectPath);
    } catch {
      continue;
    }
    for (const generation of generations) {
      agents ??= await client.agentList();
      const label = `${generation.team.team_id}:g${generation.team.generation}`;
      const alive = generation.roles.filter((role) =>
        agents?.some((agent) => agent.name === role.name && agent.pane_id === role.pane_id)
      );
      if (alive.length === 0) {
        const store = openStore(projectPath);
        try {
          const card = teamCard(store, generation.team.team_id, generation.team.generation);
          const latest = card ? latestEntry(store, card.id) : null;
          if (card && latest?.flag !== "pane:lost") {
            recordEntry(
              store,
              generation.team,
              card.id,
              `lost: no role pane of ${label} survived the Herdr restart (${generation.roles.map((role) => `${role.name} ${role.pane_id}`).join(", ")}); store: ${generation.roles.length} roles recorded`,
              "pane:lost",
              "pane.lost",
            );
          }
        } finally {
          store.close();
        }
        lines.push(`${label}: lost, every role pane is gone; noted on the team card`);
        restored.push({ generation: label, outcome: "lost" });
        continue;
      }
      const directory = slpRuntimeDirectory(projectPath, generation.team.team_id, generation.team.generation);
      if (await runtimeLockHolder(directory) !== null) {
        lines.push(`${label}: runtime already running`);
        restored.push({ generation: label, outcome: "running" });
        continue;
      }
      const paneId = await openRuntimePane(client, generation.team, alive[0]?.pane_id ?? null);
      recordRuntimePane(projectPath, generation.team.team_id, generation.team.generation, paneId);
      lines.push(`${label}: runtime pane reopened as ${paneId} (${alive.length}/${generation.roles.length} role panes alive)`);
      restored.push({ generation: label, outcome: "reopened", paneId });
    }
  }
  return {
    data: { generations: restored },
    text: lines.length > 0 ? lines.join("\n") : "no running SLP generation to restore",
  };
}

export async function slpRuntimeStatus(projectPath: string, teamId: string, generation: number): Promise<CliResult> {
  const directory = slpRuntimeDirectory(projectPath, teamId, generation);
  const holder = await runtimeLockHolder(directory);
  const state = await readRuntimeState(directory);
  const pending = Object.entries(state?.pending ?? {}).flatMap(([target, lines]) =>
    lines.map((line) => ({ line: line.line, queuedAt: line.queuedAt, subject: line.subject, target }))
  );
  const unreachable = state?.unreachable ?? [];
  return {
    data: {
      generation,
      pending,
      runtime: holder ? { pid: holder, state: "running" } : { pid: null, state: "not-running" },
      runtimePaneId: recordedRuntimePane(projectPath, teamId, generation) || null,
      supervisorPaneId: recordedSupervisorPane(projectPath, teamId, generation) || null,
      teamId,
      unreachable,
    },
    text: [
      `${teamId} g${generation} runtime ${holder ? `running (pid ${holder})` : "not running"}`,
      ...(pending.length === 0 ? ["pending: none"] : pending.map((line) => `pending ${line.target}: ${line.line}`)),
      ...unreachable.map((line) => `unreachable ${line.subject}: ${line.reason}`),
    ].join("\n"),
  };
}
