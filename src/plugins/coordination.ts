import { resolve } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { Disposer } from "../kernel/events.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { Harness, SessionRecord } from "../kernel/sessions.ts";
import type { WorkRecord, WorkService } from "./work.ts";
import { driftAdvisory } from "./lifecycle.ts";

interface MessageRow {
  id: number;
  target_session: string;
  sender_session: string;
  text: string;
  created_at: string;
}

export interface MessageRecord {
  id: number;
  targetSession: string;
  senderSession: string;
  text: string;
  createdAt: string;
}

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

export class BriefService {
  private readonly contributors: BriefContributor[] = [];

  register(contributor: BriefContributor): Disposer {
    this.contributors.push(contributor);
    return () => {
      const index = this.contributors.indexOf(contributor);
      if (index >= 0) this.contributors.splice(index, 1);
    };
  }

  async render(sessionId: string): Promise<string> {
    const sections: string[] = [];
    for (const contributor of this.contributors) {
      const section = await contributor(sessionId);
      if (section) sections.push(section);
    }
    return sections.join("\n");
  }
}

class MailboxService {
  constructor(private readonly context: PluginContext) {}

  pending(sessionId: string): number {
    const cursor = this.cursor(sessionId);
    return (
      this.context.store.database
        .query<{ count: number }, [string, number]>(
          "SELECT count(*) AS count FROM messages WHERE target_session = ? AND id > ?",
        )
        .get(sessionId, cursor)?.count ?? 0
    );
  }

  read(sessionId: string): MessageRecord[] {
    const cursor = this.cursor(sessionId);
    const messages = this.context.store.database
      .query<MessageRow, [string, number]>(
        "SELECT * FROM messages WHERE target_session = ? AND id > ? ORDER BY id",
      )
      .all(sessionId, cursor)
      .map((row) => this.fromRow(row));
    const last = messages.at(-1)?.id;
    if (last !== undefined) {
      this.context.store.database
        .query(
          `INSERT INTO message_cursors (session_id, last_message_id)
           VALUES (?, ?)
           ON CONFLICT(session_id) DO UPDATE SET last_message_id = excluded.last_message_id`,
        )
        .run(sessionId, last);
    }
    return messages;
  }

  send(targetSession: string, text: string): MessageRecord {
    const senderSession = this.context.sessions.current().id;
    const createdAt = new Date().toISOString();
    this.context.store.database
      .query(
        `INSERT INTO messages (target_session, sender_session, text, created_at)
         VALUES (?, ?, ?, ?)`,
      )
      .run(targetSession, senderSession, text, createdAt);
    const id = Number(
      this.context.store.database
        .query<{ id: number }, []>("SELECT last_insert_rowid() AS id")
        .get()?.id,
    );
    return { id, targetSession, senderSession, text, createdAt };
  }

  private cursor(sessionId: string): number {
    return (
      this.context.store.database
        .query<{ last_message_id: number }, [string]>(
          "SELECT last_message_id FROM message_cursors WHERE session_id = ?",
        )
        .get(sessionId)?.last_message_id ?? 0
    );
  }

  private fromRow(row: MessageRow): MessageRecord {
    return {
      id: row.id,
      targetSession: row.target_session,
      senderSession: row.sender_session,
      text: row.text,
      createdAt: row.created_at,
    };
  }
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function harnessOption(invocation: CliInvocation): Harness | null {
  const value = invocation.options.harness;
  if (value === undefined) return null;
  if (value === "claude" || value === "codex") return value;
  throw new CliError("INVALID_HARNESS", `invalid harness: ${String(value)}`);
}

function formatMessages(messages: MessageRecord[]): string {
  return messages
    .map((message) => `message ${message.id} from ${message.senderSession}: ${message.text}`)
    .join("\n");
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
      CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        target_session TEXT NOT NULL,
        sender_session TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS message_cursors (
        session_id TEXT PRIMARY KEY,
        last_message_id INTEGER NOT NULL
      );
    `);
    const mailbox = new MailboxService(context);
    const brief = new BriefService();
    const work = context.work as WorkService;
    context.effect(() => context.provide("mailbox", mailbox));
    context.effect(() => context.provide("brief", brief));

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
    context.effect(() =>
      brief.register((sessionId) => {
        const pending = mailbox.pending(sessionId);
        const messages = mailbox.read(sessionId);
        const count = `${pending} pending message${pending === 1 ? "" : "s"}`;
        const delivered = formatMessages(messages);
        return delivered ? `${count}\n${delivered}` : count;
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
        "msg send",
        (invocation): CliResult => {
          const target = required(invocation, 0, "target session");
          const text = required(invocation, 1, "message text");
          const message = mailbox.send(target, text);
          const sender = context.sessions.get(message.senderSession);
          const targetSession = context.sessions.get(target);
          const nativeDelivery =
            sender?.harness === "claude" &&
            targetSession?.harness === "claude" &&
            targetSession.live;
          context.log.append({
            type: "msg.send",
            entityType: "message",
            entityId: String(message.id),
            sessionId: message.senderSession,
            payload: message,
          });
          const deliveryTip = nativeDelivery
            ? `[native-delivery] also use native SendMessage for session ${target}`
            : "";
          return {
            data: { message, nativeDelivery },
            text: [`message ${message.id} sent to ${target}`, deliveryTip].filter(Boolean).join("\n"),
          };
        },
        {
          description: "Send a message to another live session.",
          positionals: [
            { name: "session", required: true },
            { name: "message", required: true },
          ],
          rootDescription: "Exchange repository-backed messages between sessions.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "msg read",
        (): CliResult => {
          const sessionId = context.sessions.current().id;
          const messages = mailbox.read(sessionId);
          if (messages.length > 0) {
            context.log.append({
              type: "msg.read",
              entityType: "session",
              entityId: sessionId,
              sessionId,
              payload: { messageIds: messages.map((message) => message.id) },
            });
          }
          return { data: { messages }, text: formatMessages(messages) };
        },
        { description: "Read new messages for the current session." },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "hook record",
        async (invocation): Promise<CliResult> => {
          const event = invocation.options.event;
          if (typeof event !== "string") {
            throw new CliError("MISSING_ARGUMENT", "missing hook event");
          }
          const harness = harnessOption(invocation);
          const session = context.sessions.record(event, harness);
          context.log.append({
            type: "hook.record",
            entityType: "session",
            entityId: session.id,
            sessionId: session.id,
            payload: { event, harness, pid: session.pid },
          });
          const text = await brief.render(session.id);
          return { data: { session, brief: text }, text };
        },
        {
          description: "Record a harness event and print the dynamic brief.",
          flags: {
            "--event": { description: "Record this harness event name.", value: true },
            "--harness": { description: "Record the originating harness.", value: true },
          },
          rootDescription: "Record harness events for session delivery.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "status",
        async (): Promise<CliResult> => {
          const sessions = context.sessions.list();
          const peers = livePeers(sessions, work.list(), context.sessions.current().id);
          const advisory = await driftAdvisory(
            process.env.HOME ?? process.cwd(),
            resolve(import.meta.dir, "..", ".."),
          );
          const sessionText =
            sessions.length > 0
              ? sessions
                  .map(
                    (session) =>
                      `${session.id} [${session.live ? "live" : "dead"}] ${session.lastEvent} pid=${session.pid} harness=${session.harness ?? "unknown"}`,
                  )
                  .join("\n")
              : "no sessions";
          return {
            data: { livePeers: peers, sessions },
            text: [sessionText, formatLivePeers(peers), advisory].filter(Boolean).join("\n"),
          };
        },
        { description: "Show sessions, live peers, and held work." },
      ),
    );
  },
};
