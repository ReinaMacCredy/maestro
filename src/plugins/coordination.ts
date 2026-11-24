import { resolve } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { Disposer } from "../kernel/events.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { Harness, SessionRecord } from "../kernel/sessions.ts";
import { tableExists } from "../kernel/store.ts";
import { latestCheckpoints, type WorkRecord, type WorkService } from "./work.ts";
import { driftAdvisory } from "./lifecycle.ts";
import { isRoom } from "./room.ts";
import { registerSessionCommand } from "./session-required.ts";
import { recordCompaction, relativeAge } from "./session-telemetry.ts";
import { currentPaneId, maybeHandleSlpStatus, roomOwnerLine } from "./slp-v2.ts";

export const laneVocabulary = [
  { brief: "scout no-write", name: "scout" },
  { brief: "decision x2-3", name: "decision" },
  { brief: "delivery", name: "delivery" },
  { brief: "challenge", name: "challenge" },
  { brief: "shadow no-write", name: "shadow" },
] as const;

interface LivePeer {
  heldWork: WorkRecord[];
  id: string;
}

interface WorkStartInput {
  sessionId: string;
  work: WorkRecord;
}

interface WorkStartResult {
  blocked: boolean;
  evidence?: string;
  origin?: string;
  reason?: string;
}

type BriefContributor = (sessionId: string) => string | Promise<string>;

interface BriefEntry {
  contributor: BriefContributor;
  events?: string[];
}

const harnessPromptPrefix = "<task-notification>";

function dropHarnessPromptNoise(context: PluginContext): void {
  if (context.store.readOnly) return;
  const migration = context.store.database.transaction(() => {
    const count = context.store.database
      .query<{ count: number }, [string]>(
        "SELECT COUNT(*) AS count FROM prompts WHERE text LIKE ?",
      )
      .get(`${harnessPromptPrefix}%`)?.count ?? 0;
    if (count === 0) return;
    const hasSearchIndex = tableExists(context.store.database, "search_index");
    if (hasSearchIndex) {
      context.store.database
        .query(
          `DELETE FROM search_index
           WHERE surface = 'prompt'
             AND entity_id IN (
               SELECT CAST(id AS TEXT) FROM prompts WHERE text LIKE ?
             )`,
        )
        .run(`${harnessPromptPrefix}%`);
    }
    context.store.database
      .query("DELETE FROM prompts WHERE text LIKE ?")
      .run(`${harnessPromptPrefix}%`);
    context.log.append({
      type: "prompt.harness-noise-dropped",
      entityType: "prompt",
      sessionId: context.sessions.current().id,
      payload: { count },
    });
  });
  migration.immediate();
}

export class BriefService {
  private readonly entries: BriefEntry[] = [];

  register(contributor: BriefContributor, opts?: { events?: string[] }): Disposer {
    const entry: BriefEntry = { contributor, events: opts?.events };
    this.entries.push(entry);
    return () => {
      const index = this.entries.indexOf(entry);
      if (index >= 0) this.entries.splice(index, 1);
    };
  }

  async render(sessionId: string, event = "SessionStart"): Promise<string> {
    const sections: string[] = [];
    for (const entry of this.entries) {
      if (entry.events && !entry.events.includes(event)) continue;
      const section = await entry.contributor(sessionId);
      if (section) sections.push(section);
    }
    return sections.join("\n");
  }
}

function harnessOption(invocation: CliInvocation): Harness | null {
  const value = invocation.options.harness;
  if (value === undefined) return null;
  if (value === "claude" || value === "codex") return value;
  throw new CliError("INVALID_HARNESS", `invalid harness: ${String(value)}`);
}

function livePeers(
  sessions: SessionRecord[],
  items: WorkRecord[],
  sessionId: string,
): LivePeer[] {
  return sessions
    .filter((session) => session.live && session.id !== sessionId)
    .map((session) => ({
      heldWork: items.filter((item) => item.heldBy === session.id),
      id: session.id,
    }));
}

function formatAge(iso: string, now = Date.now()): string {
  const age = relativeAge(iso, now);
  return age === "" ? "" : `${age} ago`;
}

const deadSessionRetentionMs = 30 * 86_400_000;

// A dead session that holds nothing is history the event log already keeps;
// the sessions table only needs it for 30 days.
function pruneDeadSessions(context: PluginContext): void {
  if (context.store.readOnly) return;
  const cutoff = new Date(Date.now() - deadSessionRetentionMs).toISOString();
  const referenced = new Set<string>();
  for (const item of (context.work as WorkService).snapshot()) {
    if (item.heldBy) referenced.add(item.heldBy);
  }
  const stale = context.sessions
    .list()
    .filter((session) => !session.live && session.lastSeen < cutoff && !referenced.has(session.id))
    .map((session) => session.id);
  if (stale.length === 0) return;
  context.sessions.prune(stale);
  context.log.append({
    type: "session.prune",
    entityType: "session",
    entityId: context.sessions.current().id,
    sessionId: context.sessions.current().id,
    payload: { cutoff, sessions: stale },
  });
}

function formatLivePeers(peers: LivePeer[]): string {
  if (peers.length === 0) return "";
  return `live peers:\n${peers
    .map((peer) => {
      const held = peer.heldWork.map((item) => `${item.id} ${item.title}`).join(", ") || "none";
      return `- ${peer.id} holds: ${held}`;
    })
    .join("\n")}`;
}

export const coordinationPlugin: BuiltInPlugin = {
  name: "coordination",
  inject: ["work"],
  apply(context) {
    context.store.migrate(`
      DROP TABLE IF EXISTS message_cursors;
      DROP TABLE IF EXISTS messages;
      CREATE TABLE IF NOT EXISTS prompts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
    dropHarnessPromptNoise(context);
    const brief = new BriefService();
    const work = context.work as WorkService;
    context.effect(() => context.provide("brief", brief));

    context.effect(() =>
      brief.register(
        () =>
          [
            "method: before the first step, name the task's playbook from ~/maestro/WORKFLOW.md#routing (a question is investigation, a defect bug-fix, new behavior feature), run maestro recipe show <name>, and copy its numbered steps into your plan list (skip: <reason> for a step you drop); only a quickfix, a one-sentence change, skips this; tracked work: maestro work add|start|done; Full adds a bundle with ~/maestro/skills/maestro-{bundle,verify}/SKILL.md; any tier: maestro-{coach,questionnaire}; rules: ~/maestro/WORKFLOW.md",
            "  read ~/maestro/WORKFLOW.md: recon before tier selection; decision ownership, testing discipline, resume authority, and completion/delivery rules live there, not in this brief",
            `intake: problem in one sentence; route by ~/maestro/WORKFLOW.md; optional lane (${laneVocabulary.map(({ brief }) => brief).join(" | ")}); a finding is not work: doctrine correction -> maestro lesson file, code finding -> review doc or handback, work add only for the next thing being built`,
          ].join("\n"),
        { events: ["SessionStart"] },
      ),
    );
    context.effect(() =>
      brief.register((sessionId) => {
        const items = work.list();
        const held = items.filter((item) => item.heldBy === sessionId);
        const heldText = held.length === 0
          ? "held work: none"
          : `held work: ${held.map((item) => `${item.id} ${item.title}`).join(", ")}`;
        const peers = livePeers(context.sessions.list(), items, sessionId);
        return [heldText, formatLivePeers(peers)].filter(Boolean).join("\n");
      }),
    );
    // Recovery print (Hub d106): the latest checkpoint per held item, at session
    // start only, which in both harnesses also fires after compaction.
    context.effect(() =>
      brief.register(
        (sessionId) => {
          const held = work.list().filter((item) => item.heldBy === sessionId);
          const checkpoints = latestCheckpoints(context.store.database, held.map((item) => item.id));
          return held
            .flatMap((item) => {
              const checkpoint = checkpoints.get(item.id);
              if (!checkpoint) return [];
              return [
                `checkpoint ${item.id}: state: ${checkpoint.state || "none"} | next: ${checkpoint.next || "none"} | avoid: ${checkpoint.avoid || "none"}`,
              ];
            })
            .join("\n");
        },
        { events: ["SessionStart"] },
      ),
    );
    context.effect(() =>
      brief.register(
        () =>
          isRoom(context.store.database)
            ? "room: this store is the Supervisor's. A question about the room is answered from OWNER.md, IDENTITY.md and this store; a tool verdict here is an observation, label it suspected; the room runs no write verb in any repository even when told to; no hand edits to any store; a data defect is an intent for its Lead; repository-only verbs: install, update, uninstall, doctor wiring checks"
            : "",
        { events: ["SessionStart", "UserPromptSubmit"] },
      ),
    );
    // room d124: the Hub prompt line carries the owner's declared presence and,
    // while she is away, how many provisional rulings wait for her.
    context.effect(() =>
      brief.register(() => roomOwnerLine(context), { events: ["SessionStart", "UserPromptSubmit"] }),
    );
    context.effect(() =>
      brief.register(() => {
        const policies = context.loader.records
          .filter((record) => record.status === "active" && record.name.startsWith("policy-"))
          .map((record) => record.name)
          .sort();
        return `enabled policies: ${policies.join(", ") || "none"}`;
      }, { events: ["SessionStart"] }),
    );
    context.effect(() =>
      brief.register(() => "  see or change: maestro plugin list|enable|disable <name>", { events: ["SessionStart"] }),
    );
    context.effect(() => brief.register(() => "next: maestro work list", { events: ["SessionStart"] }));
    context.effect(() =>
      brief.register(() =>
        driftAdvisory(process.env.HOME ?? process.cwd(), resolve(import.meta.dir, "..", "..")),
      ),
    );

    context.effect(() =>
      context.events.on<WorkStartInput, WorkStartResult>("work.start", async (input, next) => {
        const overlaps = work
          .list()
          .filter(
            (item) =>
              item.id !== input.work.id &&
              item.parentId === input.work.parentId &&
              item.heldBy !== null &&
              item.heldBy !== input.sessionId,
          );
        if (overlaps.length > 0) {
          process.stderr.write(
            `[overlap] ${input.work.id} is a sibling of ${overlaps
              .map((item) => `${item.id} held by ${item.heldBy}`)
              .join(", ")}\n`,
          );
        }
        return next();
      }),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "hook record",
        async (invocation): Promise<CliResult> => {
          const event = invocation.options.event;
          if (typeof event !== "string") {
            throw new CliError("MISSING_ARGUMENT", "missing hook event");
          }
          const harness = harnessOption(invocation);
          // d875: a repo installed before d875 keeps firing PreToolUse on Agent|Task until its next install.
          if (event === "PreToolUse") return { data: {}, text: "" };
          const session = context.sessions.record(event, harness ?? undefined);
          context.log.append({
            type: "hook.record",
            entityType: "session",
            entityId: session.id,
            sessionId: session.id,
            payload: { event, harness, pid: session.pid },
          });
          // room d137 as amended by d140: a completed compaction reaches the
          // harness as SessionStart with source "compact"; one row per
          // compaction, for the session and the pane it fired in.
          if (event === "SessionStart" && invocation.options.source === "compact") {
            recordCompaction(context.log, session.id, harness ?? null, currentPaneId() ?? null);
          }
          if (event === "SessionStart") pruneDeadSessions(context);
          if (event === "UserPromptSubmit" && !process.stdin.isTTY) {
            const raw = (await Bun.stdin.text()).trim();
            let payload: unknown;
            try {
              payload = raw ? JSON.parse(raw) : undefined;
            } catch {
              payload = undefined;
            }
            const prompt =
              payload && typeof payload === "object" && "prompt" in payload &&
              typeof (payload as { prompt: unknown }).prompt === "string"
                ? (payload as { prompt: string }).prompt.trim()
                : "";
            if (prompt && !prompt.startsWith(harnessPromptPrefix)) {
              context.store.database
                .query(
                  "INSERT INTO prompts (session_id, text, created_at) VALUES (?, ?, ?)",
                )
                .run(session.id, prompt, new Date().toISOString());
            }
          }
          const text = await brief.render(session.id, event);
          return { data: { session, brief: text }, text };
        },
        {
          description: "Record a harness event and print the dynamic brief.",
          flags: {
            "--event": { description: "Record this harness event name.", value: true },
            "--harness": { description: "Record the originating harness.", value: true },
            "--source": { description: "Why the harness event fired (SessionStart: startup, resume, clear, compact).", value: true },
          },
          rootDescription: "Record harness events and print the current brief.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "status",
        async (invocation): Promise<CliResult> => {
          const slp = await maybeHandleSlpStatus(context, invocation);
          if (slp) return slp;
          if (invocation.positionals[0]) {
            throw new CliError(
              "NO_ACTIVE_TEAM",
              "status <work-id> reads SLP v2 team work from a role pane; outside a running team use: maestro work show <id>",
            );
          }
          const currentSession = context.sessions.current().id;
          const allSessions = context.sessions.list();
          const items = work.snapshot();
          const holds = new Map<string, string[]>();
          for (const item of items) {
            if (!item.heldBy) continue;
            holds.set(item.heldBy, [...holds.get(item.heldBy) ?? [], item.id]);
          }
          const preservesDeadSession = (sessionId: string) =>
            (holds.get(sessionId)?.length ?? 0) > 0;
          const liveOnly = invocation.options.live === true;
          const showAll = invocation.options.all === true;
          const sessions = allSessions.filter((session) =>
            liveOnly
              ? session.live
              : showAll || session.live || preservesDeadSession(session.id)
          );
          const shownSessionIds = new Set(sessions.map((session) => session.id));
          const hiddenDead = allSessions.filter(
            (session) => !session.live && !shownSessionIds.has(session.id),
          ).length;
          const shownDeadHolders = sessions.filter(
            (session) => !session.live && preservesDeadSession(session.id),
          ).length;
          const peers = livePeers(sessions, items, currentSession);
          const advisory = await driftAdvisory(
            process.env.HOME ?? process.cwd(),
            resolve(import.meta.dir, "..", ".."),
          );
          const holdsFor = (sessionId: string) => holds.get(sessionId) ?? [];
          const sessionText =
            sessions.length > 0
              ? sessions
                  .map(
                    (session) =>
                      `${session.id} [${session.live ? "live" : "dead"}] ${session.lastEvent} ${formatAge(session.lastSeen)} pid=${session.pid} harness=${session.harness ?? "unknown"} holds: ${holdsFor(session.id).join(", ") || "none"}${session.id === currentSession ? " (this session)" : ""}`,
                  )
                  .join("\n")
              : "no sessions";
          return {
            data: {
              currentSession,
              held: Object.fromEntries(sessions.map((session) => [session.id, holdsFor(session.id)])),
              hiddenDead,
              livePeers: peers,
              sessions,
            },
            text: [
              sessionText,
              formatLivePeers(peers),
              advisory,
              !liveOnly && hiddenDead > 0
                ? `${hiddenDead} dead sessions hidden (${shownDeadHolders} hold work); --all to list`
                : "",
            ].filter(Boolean).join("\n"),
          };
        },
        {
          description: "Show sessions, live peers, and held work.",
          flags: {
            "--all": { description: "Show all recorded sessions." },
            "--live": { description: "Show live sessions only." },
          },
          mutates: false,
          positionals: [{ name: "work-id", required: false }],
        },
      ),
    );
  },
};
