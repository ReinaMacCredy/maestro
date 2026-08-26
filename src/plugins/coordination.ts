import { resolve } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { Disposer } from "../kernel/events.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { Harness, SessionRecord } from "../kernel/sessions.ts";
import type { WorkRecord, WorkService } from "./work.ts";
import { driftAdvisory } from "./lifecycle.ts";
import { registerSessionCommand } from "./session-required.ts";

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

interface PromptRecord {
  created_at: string;
  id: number;
  session_id: string;
  text: string;
}

interface BriefEntry {
  contributor: BriefContributor;
  events?: string[];
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
    const brief = new BriefService();
    const work = context.work as WorkService;
    context.effect(() => context.provide("brief", brief));

    context.effect(() =>
      brief.register(
        () =>
          [
            "method: design -> work -> verify; skills: ~/maestro/skills/maestro-{bundle,design,work,verify}/SKILL.md",
            "  tier: one session, one branch, acceptance in a sentence -> maestro work add|start|done",
            "        multi-session, shared scope, high risk, or repeat fix -> maestro bundle open <id> --work <id>",
            '  forks: settle before tests - maestro decision draft "<choice>" --rationale "<why + rejected alternative>", then decision lock',
            '  close: maestro bundle close <id> after VERIFY passes; recall with maestro search "<term>"',
            "intake: problem in one sentence; uncertainty -> lane (scout no-write | decision x2-3 | delivery | challenge); ROI 0-10 -> tier",
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
    context.effect(() =>
      brief.register(() => {
        const policies = context.loader.records
          .filter((record) => record.status === "active" && record.name.startsWith("policy-"))
          .map((record) => record.name)
          .sort();
        return `enabled policies: ${policies.join(", ") || "none"}`;
      }),
    );
    context.effect(() => brief.register(() => "next: maestro ready"));
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
      context.cli.register(
        "prompt list",
        (invocation): CliResult => {
          const session = invocation.options.session;
          const rows = (
            typeof session === "string"
              ? context.store.database
                  .query<PromptRecord, [string]>(
                    "SELECT id, session_id, text, created_at FROM prompts WHERE session_id = ? ORDER BY id DESC LIMIT 20",
                  )
                  .all(session)
              : context.store.database
                  .query<PromptRecord, []>(
                    "SELECT id, session_id, text, created_at FROM prompts ORDER BY id DESC LIMIT 20",
                  )
                  .all()
          );
          const lines = rows.map(
            (row) =>
              `p${row.id} [${row.session_id}] ${row.text.replaceAll(/\s+/g, " ").slice(0, 200)}`,
          );
          return {
            data: { prompts: rows },
            text: lines.join("\n") || "no prompts recorded",
          };
        },
        {
          description: "List recorded user prompts, most recent first (20).",
          flags: {
            "--session": { description: "Only prompts from this session.", value: true },
          },
          rootDescription: "Recorded user prompts from harness hooks.",
        },
      ),
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
          const session = context.sessions.record(event, harness ?? undefined);
          context.log.append({
            type: "hook.record",
            entityType: "session",
            entityId: session.id,
            sessionId: session.id,
            payload: { event, harness, pid: session.pid },
          });
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
            if (prompt) {
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
          },
          rootDescription: "Record harness events and print the current brief.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "status",
        async (invocation): Promise<CliResult> => {
          const sessions = context.sessions
            .list()
            .filter((session) => invocation.options.live !== true || session.live);
          const items = work.list();
          const peers = livePeers(sessions, items, context.sessions.current().id);
          const advisory = await driftAdvisory(
            process.env.HOME ?? process.cwd(),
            resolve(import.meta.dir, "..", ".."),
          );
          const holdsFor = (sessionId: string) =>
            items.filter((item) => item.heldBy === sessionId).map((item) => item.id);
          const sessionText =
            sessions.length > 0
              ? sessions
                  .map(
                    (session) =>
                      `${session.id} [${session.live ? "live" : "dead"}] ${session.lastEvent} pid=${session.pid} harness=${session.harness ?? "unknown"} holds: ${holdsFor(session.id).join(", ") || "none"}`,
                  )
                  .join("\n")
              : "no sessions";
          return {
            data: {
              held: Object.fromEntries(sessions.map((session) => [session.id, holdsFor(session.id)])),
              livePeers: peers,
              sessions,
            },
            text: [sessionText, formatLivePeers(peers), advisory].filter(Boolean).join("\n"),
          };
        },
        {
          description: "Show sessions, live peers, and held work.",
          flags: { "--live": { description: "Show live sessions only." } },
        },
      ),
    );
  },
};
