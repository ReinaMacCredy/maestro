import { afterEach } from "bun:test";
import { appendFile, chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { materializeProfiles } from "../src/plugins/profiles.ts";
import type { Fixture } from "./helpers.ts";

// A socket fake of Herdr 0.8.2 (protocol 20): newline JSON over a unix
// socket, one request per connection, events.subscribe kept open. Every
// request is logged in the argv shape the old CLI fake recorded, so the
// suite's assertions read the same. A `herdr` script on the fixture PATH is
// the tripwire: the CLI must never be spawned once SLP speaks the socket.

export interface FakeHerdrBehavior {
  acknowledgementPrefixes?: Partial<Record<"team-supervisor" | "lead" | "peer", string>>;
  acknowledgementDelayReads?: number;
  // A started agent stays off agent.list, and prompts to it fail with
  // agent_not_ready, for this many list reads: the socket start answered
  // before the name was active (live 2026-09-05).
  agentActivationDelayReads?: number;
  agentBusyAttempts?: number;
  agentStartDelayMs?: number;
  agents?: boolean;
  closeResources?: boolean;
  closeWorkspaceWithLastTab?: boolean;
  closeWorkspace?: boolean;
  codexNotReadyAttempts?: number;
  failWorkspaceId?: string;
  invalidAcknowledgementField?: "challenge" | "generation";
  spacedChallenge?: boolean;
  processInfo?: boolean;
  processInfoDelayMs?: number;
  // Every accepted prompt pushes working then idle for the target's pane.
  promptEvents?: boolean;
  // The first prompts fail with agent_not_ready although the agent is listed.
  promptNotReadyAttempts?: number;
  promptStalledAttempts?: number;
  prompts?: boolean;
  // Herdr replays every past event to a new subscriber; false skips it.
  replayOnSubscribe?: boolean;
  // spawn: plugin.pane.open runs `maestro slp runtime` as a child process;
  // record: the pane and its process entry appear in the state only.
  runtimePane?: "record" | "spawn";
  settleAgents?: boolean;
  trustDialog?: "claude" | "codex";
  wrapAcknowledgements?: boolean;
  workspaceCloseListLag?: number;
  workspaceListDelayMs?: number;
}

export interface FakeHerdrFixture {
  env: Record<string, string>;
  log: string;
  socket: string;
  tripwire: string;
}

export interface FakeHerdrEvent {
  data: Record<string, unknown>;
  event: "pane_agent_detected" | "pane_agent_status_changed" | "pane_closed" | "pane_created" | "pane_exited";
}

type Params = Record<string, any>;

interface Subscriber {
  socket: FakeSocket;
  subscriptions: Params[];
}

interface FakeSocket {
  end(): void;
  write(data: string): number;
}

class FakeHerdrError extends Error {
  constructor(readonly code: string, message: string) {
    super(message);
  }
}

interface FakeServer {
  children: Map<string, ReturnType<typeof Bun.spawn>>;
  fixture: Fixture;
  listener: { stop(closeActive?: boolean): void };
  logChain: Promise<void>;
  queue: Promise<unknown>;
  state: Params;
  subscribers: Set<Subscriber>;
}

const servers = new Map<string, FakeServer>();
const cliEntry = join(import.meta.dir, "..", "bin", "maestro.ts");
const knownMethods = [
  "agent.get", "agent.list", "agent.prompt", "agent.read", "agent.start",
  "events.subscribe",
  "pane.close", "pane.get", "pane.list", "pane.process_info", "pane.read", "pane.send_input",
  "ping",
  "plugin.link", "plugin.list", "plugin.pane.open", "plugin.unlink",
  "tab.close", "tab.create", "tab.list",
  "workspace.close", "workspace.create", "workspace.list",
];

afterEach(() => stopFakeHerdrServers());

export function stopFakeHerdrServers(): void {
  for (const [socket, server] of servers) {
    for (const child of server.children.values()) child.kill();
    server.children.clear();
    for (const subscriber of server.subscribers) subscriber.socket.end();
    server.subscribers.clear();
    server.listener.stop(true);
    servers.delete(socket);
  }
}

function requireServer(fake: FakeHerdrFixture): FakeServer {
  const server = servers.get(fake.socket);
  if (!server) throw new Error(`fake Herdr is not running at ${fake.socket}`);
  return server;
}

const tripwireSource = (log: string) => `#!/usr/bin/env bun
import { appendFile } from "node:fs/promises";
const args = process.argv.slice(2);
await appendFile(${JSON.stringify(log)}, JSON.stringify(args) + "\\n");
process.stderr.write("tripwire: herdr CLI spawned in tests: " + args.join(" ") + "\\n");
process.exit(64);
`;

// The log keeps the CLI argv shape the suite asserts on.
function commandShape(method: string, params: Params): string[] {
  const flag = (name: string, value: unknown): string[] =>
    value === undefined || value === null ? [] : [name, String(value)];
  const noFocus = params.focus ? [] : ["--no-focus"];
  switch (method) {
    case "workspace.list":
      return ["workspace", "list"];
    case "workspace.create":
      return ["workspace", "create", ...flag("--cwd", params.cwd), ...flag("--label", params.label), ...noFocus];
    case "workspace.close":
      return ["workspace", "close", params.workspace_id];
    case "tab.list":
      return ["tab", "list", ...flag("--workspace", params.workspace_id)];
    case "tab.create":
      return [
        "tab", "create", ...flag("--workspace", params.workspace_id), ...flag("--cwd", params.cwd),
        ...flag("--label", params.label), ...noFocus,
      ];
    case "tab.close":
      return ["tab", "close", params.tab_id];
    case "pane.list":
      return ["pane", "list", ...flag("--workspace", params.workspace_id)];
    case "pane.get":
      return ["pane", "get", params.pane_id];
    case "pane.close":
      return ["pane", "close", params.pane_id];
    case "pane.process_info":
      return ["pane", "process-info", ...flag("--pane", params.pane_id)];
    case "pane.read":
      return ["pane", "read", params.pane_id, ...flag("--source", params.source), ...flag("--lines", params.lines)];
    case "pane.send_input":
      return ["pane", "run", params.pane_id, ...String(params.text ?? "").split(" ")];
    case "agent.list":
      return ["agent", "list"];
    case "agent.get":
      return ["agent", "get", params.target];
    case "agent.read":
      return [
        "agent", "read", params.target, ...flag("--source", params.source), ...flag("--lines", params.lines),
        ...flag("--format", params.format),
      ];
    case "agent.start":
      return [
        "agent", "start", params.name, ...flag("--kind", params.kind), ...flag("--pane", params.pane_id),
        ...flag("--timeout", params.timeout_ms), ...(Array.isArray(params.args) && params.args.length > 0 ? ["--", ...params.args] : []),
      ];
    case "agent.prompt":
      return [
        "agent", "prompt", params.target, params.text,
        ...(params.wait ? ["--wait", ...flag("--timeout", params.wait.timeout_ms)] : []),
      ];
    case "events.subscribe":
      return ["events", "subscribe", ...(params.subscriptions ?? []).map((subscription: Params) =>
        subscription.pane_id ? `${subscription.type}:${subscription.pane_id}` : String(subscription.type))];
    case "plugin.list":
      return ["plugin", "list"];
    case "plugin.link":
      return ["plugin", "link", params.path];
    case "plugin.unlink":
      return ["plugin", "unlink", params.plugin_id];
    case "plugin.pane.open":
      return [
        "plugin", "pane", "open", ...flag("--plugin", params.plugin_id), ...flag("--entrypoint", params.entrypoint),
        ...flag("--placement", params.placement), ...flag("--workspace", params.workspace_id),
        ...flag("--target-pane", params.target_pane_id), ...flag("--cwd", params.cwd),
        ...Object.entries(params.env ?? {}).flatMap(([key, value]) => ["--env", `${key}=${value}`]),
        ...noFocus,
      ];
    default:
      return [method];
  }
}

function eventWanted(subscriber: Subscriber, event: FakeHerdrEvent): boolean {
  return subscriber.subscriptions.some((subscription) => {
    const type = String(subscription.type).replace(".", "_");
    if (type !== event.event) return false;
    const paneId = event.data.pane_id ?? (event.data.pane as { pane_id?: string } | undefined)?.pane_id;
    if (subscription.pane_id !== undefined && subscription.pane_id !== paneId) return false;
    if (subscription.agent_status && subscription.agent_status !== event.data.agent_status) return false;
    return true;
  });
}

function pushEvent(server: FakeServer, event: FakeHerdrEvent): number {
  let delivered = 0;
  const line = `${JSON.stringify(event)}\n`;
  server.state.history.push(event);
  for (const subscriber of server.subscribers) {
    const wanted = eventWanted(subscriber, event);
    if (!wanted) continue;
    subscriber.socket.write(line);
    delivered += 1;
  }
  return delivered;
}

function statusEvent(server: FakeServer, agent: Params, status: string): void {
  agent.agent_status = status;
  pushEvent(server, {
    event: "pane_agent_status_changed",
    data: {
      agent: agent.kind ?? null,
      agent_status: status,
      display_agent: agent.kind ?? null,
      pane_id: agent.pane_id,
      state_labels: {},
      title: null,
      workspace_id: agent.workspace_id,
    },
  });
}

function closePanes(server: FakeServer, paneIds: Iterable<string>): void {
  const state = server.state;
  for (const paneId of paneIds) {
    const child = server.children.get(paneId);
    if (child) {
      child.kill();
      server.children.delete(paneId);
    }
    delete state.processes[paneId];
    state.agents = state.agents.filter((candidate: Params) => candidate.pane_id !== paneId);
    const pane = state.panes.find((candidate: Params) => candidate.pane_id === paneId);
    state.panes = state.panes.filter((candidate: Params) => candidate.pane_id !== paneId);
    pushEvent(server, { event: "pane_closed", data: { pane_id: paneId, workspace_id: pane?.workspace_id ?? "" } });
  }
}

function spawnRuntimePane(server: FakeServer, paneId: string, params: Params): void {
  const pane = server.state.panes.find((candidate: Params) => candidate.pane_id === paneId) as Params;
  const environment: Record<string, string | undefined> = { ...process.env };
  for (const key of Object.keys(environment)) if (key.startsWith("HERDR_")) delete environment[key];
  const stateDirectory = join(server.fixture.root, "herdr-plugin-state");
  const child = Bun.spawn([process.execPath, cliEntry, "slp", params.entrypoint], {
    cwd: params.cwd ?? server.fixture.repo,
    env: {
      ...environment,
      HERDR_ENV: "1",
      HERDR_PANE_ID: paneId,
      HERDR_PLUGIN_ENTRYPOINT_ID: params.entrypoint,
      HERDR_PLUGIN_ID: params.plugin_id,
      HERDR_PLUGIN_STATE_DIR: stateDirectory,
      HERDR_SOCKET_PATH: join(server.fixture.root, "herdr.sock"),
      HERDR_TAB_ID: pane.tab_id,
      HERDR_WORKSPACE_ID: pane.workspace_id,
      HOME: server.fixture.home,
      ...(params.env ?? {}),
    },
    stderr: "pipe",
    stdout: "ignore",
  });
  server.children.set(paneId, child);
  child.exited.then(async () => {
    const stderr = await new Response(child.stderr as ReadableStream<Uint8Array>).text().catch(() => "");
    if (server.children.get(paneId) !== child) return;
    server.children.delete(paneId);
    delete server.state.processes[paneId];
    server.state.runtime_exits.push({ code: child.exitCode, pane_id: paneId, stderr });
    pushEvent(server, { event: "pane_exited", data: { pane_id: paneId, workspace_id: pane.workspace_id } });
  });
}

async function handle(server: FakeServer, method: string, params: Params, subscriber: Subscriber): Promise<Params> {
  const state = server.state;
  const behavior = state.behavior as FakeHerdrBehavior;
  const next = (prefix: string) => prefix + String(++state.sequence);
  const pane = (paneId: string): Params | undefined =>
    state.panes.find((candidate: Params) => candidate.pane_id === paneId);
  const agentByTarget = (target: string): Params | undefined =>
    state.agents.find((candidate: Params) => candidate.name === target || candidate.pane_id === target);
  if (!knownMethods.includes(method)) {
    throw new FakeHerdrError(
      "invalid_request",
      `invalid request: unknown variant \`${method}\`, expected one of ${knownMethods.map((known) => `\`${known}\``).join(", ")}`,
    );
  }
  switch (method) {
    case "ping":
      return { type: "pong", version: "0.8.2", protocol: state.protocol, capabilities: { live_handoff: true } };
    case "workspace.list": {
      if (behavior.workspaceListDelayMs) await Bun.sleep(behavior.workspaceListDelayMs);
      for (const [workspaceId, remaining] of Object.entries(state.pending_workspace_closes as Record<string, number>)) {
        if (remaining <= 0) {
          state.workspaces = state.workspaces.filter((candidate: Params) => candidate.workspace_id !== workspaceId);
          delete state.pending_workspace_closes[workspaceId];
        } else {
          state.pending_workspace_closes[workspaceId] = remaining - 1;
        }
      }
      return { type: "workspace_list", workspaces: state.workspaces };
    }
    case "workspace.create": {
      const workspaceId = next("w");
      const tabId = `${workspaceId}:${next("t")}`;
      const paneId = `${workspaceId}:${next("p")}`;
      const workspace = { workspace_id: workspaceId, cwd: params.cwd, label: params.label };
      const tab = { tab_id: tabId, workspace_id: workspaceId, root_pane_id: paneId, label: "1" };
      const rootPane = { pane_id: paneId, workspace_id: workspaceId, tab_id: tabId, cwd: params.cwd, label: params.label };
      state.workspaces.push(workspace);
      state.tabs.push(tab);
      state.panes.push(rootPane);
      return { type: "workspace_created", workspace, tab, root_pane: rootPane };
    }
    case "workspace.close": {
      const workspaceId = params.workspace_id;
      if (!state.workspaces.some((candidate: Params) => candidate.workspace_id === workspaceId)) {
        throw new FakeHerdrError("workspace_not_found", `workspace not found: ${workspaceId}`);
      }
      const closed = behavior.closeResources !== false && behavior.closeWorkspace !== false &&
        behavior.failWorkspaceId !== workspaceId;
      if (closed) {
        const paneIds = state.panes
          .filter((candidate: Params) => candidate.workspace_id === workspaceId)
          .map((candidate: Params) => candidate.pane_id as string);
        if ((behavior.workspaceCloseListLag ?? 0) > 0) {
          state.pending_workspace_closes[workspaceId] = behavior.workspaceCloseListLag;
          behavior.workspaceCloseListLag = 0;
        } else {
          state.workspaces = state.workspaces.filter((candidate: Params) => candidate.workspace_id !== workspaceId);
        }
        state.tabs = state.tabs.filter((candidate: Params) => candidate.workspace_id !== workspaceId);
        closePanes(server, paneIds);
      }
      if (!closed) throw new FakeHerdrError("workspace_close_failed", `workspace ${workspaceId} did not close`);
      return { type: "ok" };
    }
    case "tab.list":
      return {
        type: "tab_list",
        tabs: state.tabs.filter((candidate: Params) => !params.workspace_id || candidate.workspace_id === params.workspace_id),
      };
    case "tab.create": {
      const workspaceId = params.workspace_id;
      const tabId = `${workspaceId}:${next("t")}`;
      const paneId = `${tabId}:${next("p")}`;
      const tab = { tab_id: tabId, workspace_id: workspaceId, root_pane_id: paneId, label: params.label };
      const rootPane = { pane_id: paneId, workspace_id: workspaceId, tab_id: tabId, cwd: params.cwd, label: params.label };
      state.tabs.push(tab);
      state.panes.push(rootPane);
      pushEvent(server, { event: "pane_created", data: { pane: rootPane } });
      return { type: "tab_created", tab, root_pane: rootPane };
    }
    case "tab.close": {
      const tabId = params.tab_id;
      const targetTab = state.tabs.find((candidate: Params) => candidate.tab_id === tabId);
      if (!targetTab) throw new FakeHerdrError("tab_not_found", `tab ${tabId} not found`);
      const closed = behavior.closeResources !== false;
      if (closed) {
        const paneIds = state.panes
          .filter((candidate: Params) => candidate.tab_id === tabId)
          .map((candidate: Params) => candidate.pane_id as string);
        state.tabs = state.tabs.filter((candidate: Params) => candidate.tab_id !== tabId);
        closePanes(server, paneIds);
        if (
          behavior.closeWorkspaceWithLastTab &&
          !state.tabs.some((candidate: Params) => candidate.workspace_id === targetTab.workspace_id)
        ) {
          state.workspaces = state.workspaces.filter((candidate: Params) => candidate.workspace_id !== targetTab.workspace_id);
        }
      }
      if (!closed) throw new FakeHerdrError("tab_close_failed", `tab ${tabId} did not close`);
      return { type: "ok" };
    }
    case "pane.list":
      return {
        type: "pane_list",
        panes: state.panes.filter((candidate: Params) => !params.workspace_id || candidate.workspace_id === params.workspace_id),
      };
    case "pane.get": {
      const target = pane(params.pane_id);
      if (!target) throw new FakeHerdrError("pane_not_found", `pane ${params.pane_id} not found`);
      return { type: "pane_info", pane: target };
    }
    case "pane.close": {
      const paneId = params.pane_id;
      const closed = behavior.closeResources !== false;
      if (closed) {
        const target = pane(paneId);
        closePanes(server, [paneId]);
        if (target?.tab_id) {
          const remaining = state.panes.filter((candidate: Params) => candidate.tab_id === target.tab_id);
          const tab = state.tabs.find((candidate: Params) => candidate.tab_id === target.tab_id);
          if (remaining.length === 0) {
            state.tabs = state.tabs.filter((candidate: Params) => candidate.tab_id !== target.tab_id);
          } else if (tab?.root_pane_id === paneId) {
            tab.root_pane_id = remaining[0].pane_id;
          }
        }
      }
      if (!closed) throw new FakeHerdrError("pane_close_failed", `pane ${paneId} did not close`);
      return { type: "ok" };
    }
    case "pane.process_info": {
      if (behavior.processInfoDelayMs) await Bun.sleep(behavior.processInfoDelayMs);
      if (behavior.processInfo === false) throw new FakeHerdrError("internal", "injected process-info failure");
      const paneId = params.pane_id;
      const target = pane(paneId);
      return {
        type: "pane_process_info",
        process_info: state.processes[paneId] ?? {
          pane_id: paneId,
          cwd: target?.cwd,
          shell_pid: 1000 + state.sequence,
          foreground_process_group_id: null,
          foreground_processes: [],
        },
      };
    }
    case "pane.send_input": {
      const paneId = params.pane_id;
      const target = pane(paneId);
      if (!target) throw new FakeHerdrError("pane_not_found", `pane ${paneId} not found`);
      const words = String(params.text ?? "").split(" ");
      if (words[0] === "/usr/bin/env" && words.some((word) => word.startsWith("MAESTRO_SLP_STOP_GRANT="))) {
        const extraEnvironment: Record<string, string> = {};
        let commandIndex = 1;
        while (commandIndex < words.length && (words[commandIndex] as string).includes("=")) {
          const assignment = words[commandIndex] as string;
          const separator = assignment.indexOf("=");
          extraEnvironment[assignment.slice(0, separator)] = assignment.slice(separator + 1);
          commandIndex += 1;
        }
        const environment: Record<string, string | undefined> = { ...process.env };
        for (const key of Object.keys(environment)) if (key.startsWith("HERDR_")) delete environment[key];
        const child = Bun.spawn(words.slice(commandIndex), {
          cwd: target.cwd,
          env: {
            ...environment,
            ...extraEnvironment,
            HERDR_PANE_ID: paneId,
            HERDR_SOCKET_PATH: join(server.fixture.root, "herdr.sock"),
            HERDR_WORKSPACE_ID: target.workspace_id,
            HOME: server.fixture.home,
          },
          stderr: "pipe",
          stdout: "pipe",
        });
        server.children.set(`helper:${paneId}`, child);
        child.exited.then(async () => {
          server.children.delete(`helper:${paneId}`);
          const stderr = await new Response(child.stderr as ReadableStream<Uint8Array>).text().catch(() => "");
          state.helper_exits.push({ code: child.exitCode, pane_id: paneId, stderr });
        });
      } else {
        state.processes[paneId] = {
          pane_id: paneId,
          cwd: target.cwd,
          shell_pid: 1000 + state.sequence,
          foreground_process_group_id: 2000 + state.sequence,
          foreground_processes: [{ pid: 2000 + state.sequence, name: words[0], command: words.join(" "), args: words }],
        };
      }
      return { type: "ok" };
    }
    case "agent.list": {
      const hidden = state.activating as Record<string, number>;
      for (const name of Object.keys(hidden)) {
        hidden[name] = (hidden[name] as number) - 1;
        if ((hidden[name] as number) <= 0) delete hidden[name];
      }
      return { type: "agent_list", agents: state.agents.filter((candidate: Params) => !(candidate.name in hidden)) };
    }
    case "agent.get": {
      const agent = agentByTarget(params.target);
      if (!agent) throw new FakeHerdrError("agent_not_found", `agent ${params.target} not found`);
      return { type: "agent_info", agent };
    }
    case "agent.start": {
      const { name, kind = "codex", pane_id: paneId } = params;
      if (behavior.agentStartDelayMs) await Bun.sleep(behavior.agentStartDelayMs);
      if ((behavior.agentBusyAttempts ?? 0) > 0) {
        behavior.agentBusyAttempts = (behavior.agentBusyAttempts as number) - 1;
        throw new FakeHerdrError("agent_pane_busy", `agent target pane ${paneId} is not an available shell`);
      }
      if (behavior.agents === false) throw new FakeHerdrError("agent_start_failed", `agent ${name} was not started`);
      let agent = state.agents.find((candidate: Params) => candidate.name === name);
      if (!agent) {
        const target = pane(paneId);
        agent = { name, pane_id: paneId, workspace_id: target?.workspace_id, agent_status: "working", kind };
        state.agents.push(agent);
        state.processes[paneId] = {
          pane_id: paneId,
          cwd: target?.cwd,
          shell_pid: 3000 + state.sequence,
          foreground_process_group_id: 4000 + state.sequence,
          foreground_processes: [{ pid: 4000 + state.sequence, name: kind, command: kind, args: [kind] }],
        };
        // Herdr announces a detected agent before any status subscription can
        // name its pane; the runtime learns new Peer panes from this.
        pushEvent(server, {
          event: "pane_agent_detected",
          data: { agent: kind, final_status: null, pane_id: paneId, released: false, workspace_id: target?.workspace_id ?? "" },
        });
      }
      if (behavior.trustDialog && kind === behavior.trustDialog) {
        agent.agent_status = "blocked";
        const cwd = pane(paneId)?.cwd ?? "";
        state.outputs[name] = kind === "claude"
          ? ` Accessing workspace:\n\n ${cwd}\n\n Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source project, or work from your team).\n\n Claude Code'll be able to read, edit, and execute files here.\n\n ❯ No, exit\n   Yes, I trust this folder\n\n Enter to confirm · Esc to cancel\n`
          : `> You are in ${cwd}\n\n  Do you trust the contents of this directory? Working with untrusted contents comes with higher risk of prompt injection.\n\n› 1. Yes, continue\n  2. No, quit\n\n  Press enter to continue\n`;
        throw new FakeHerdrError("agent_not_ready", `agent ${name} is blocked during startup and is not ready for prompts`);
      }
      if (kind === "codex" && (behavior.codexNotReadyAttempts ?? 0) > 0) {
        behavior.codexNotReadyAttempts = (behavior.codexNotReadyAttempts as number) - 1;
        agent.agent_status = "idle";
        throw new FakeHerdrError("agent_not_ready", `agent ${name} is blocked during startup and is not ready for prompts`);
      }
      if ((behavior.agentActivationDelayReads ?? 0) > 0) {
        state.activating[name] = behavior.agentActivationDelayReads;
        return {
          type: "agent_started",
          agent: { ...agent, interactive_ready: false, launch_pending: true },
          argv: [kind, ...(params.args ?? [])],
        };
      }
      return {
        type: "agent_started",
        agent: { ...agent, interactive_ready: true, launch_pending: false },
        argv: [kind, ...(params.args ?? [])],
      };
    }
    case "agent.prompt": {
      const target = params.target;
      if ((behavior.promptStalledAttempts ?? 0) > 0) {
        behavior.promptStalledAttempts = (behavior.promptStalledAttempts as number) - 1;
        throw new FakeHerdrError("agent_prompt_stalled", "agent prompt produced no observed state change within 5000 ms");
      }
      if (behavior.prompts === false) throw new FakeHerdrError("agent_not_found", `agent ${target} not found`);
      if (target in state.activating || (behavior.promptNotReadyAttempts ?? 0) > 0) {
        if (!(target in state.activating)) behavior.promptNotReadyAttempts = (behavior.promptNotReadyAttempts as number) - 1;
        throw new FakeHerdrError("agent_not_ready", `agent ${target} is not an active named agent`);
      }
      const body = params.text ?? "";
      state.prompts.push({ name: target, body });
      // d90: the post-open prompt is one line; the seat knows its role from its
      // rendered profile, which the fake reads off the agent name prefix.
      const opened = /^slp team (\S+) generation (\d+) instance \S+; reply ([0-9a-f]{32})$/.exec(body);
      const role = target.startsWith("supervisor-") ? "team-supervisor" : target.startsWith("lead-") ? "lead" : "peer";
      if (opened) {
        const [, team, generation, challenge] = opened as unknown as [string, string, string, string];
        const challengeLeft = challenge.slice(0, 16);
        const challengeRight = challenge.slice(16);
        const acknowledgement = [
          "SLP_ROLE_READY",
          `team=${team}`,
          `generation=${generation}`,
          `role=${role}`,
          `challenge=${challengeLeft}${challengeRight}`,
        ];
        if (behavior.invalidAcknowledgementField === "generation") acknowledgement[2] = "generation=0";
        if (behavior.invalidAcknowledgementField === "challenge") acknowledgement[4] = `challenge=${"0".repeat(32)}`;
        if (behavior.spacedChallenge) acknowledgement[4] = `challenge=${challengeLeft} ${challengeRight}`;
        const lines = behavior.wrapAcknowledgements
          ? [acknowledgement.slice(0, 3).join(" "), acknowledgement.slice(3, 4).join(" "), acknowledgement.slice(4).join(" ")]
          : [acknowledgement.join(" ")];
        state.outputs[target] = `${behavior.acknowledgementPrefixes?.[role] ?? ""}${lines.join("\n")}\n`;
      }
      const agent = agentByTarget(target);
      if (agent && behavior.promptEvents) statusEvent(server, agent, "working");
      if (agent && behavior.settleAgents !== false) {
        if (behavior.promptEvents) statusEvent(server, agent, "idle");
        else agent.agent_status = "idle";
      }
      return { type: "agent_prompted", agent: agent ?? { name: target, agent_status: "idle" } };
    }
    case "agent.read": {
      const target = params.target;
      const agent = agentByTarget(target);
      const name = agent?.name ?? target;
      let text: string;
      if ((behavior.acknowledgementDelayReads ?? 0) > 0 && (state.outputs[name] ?? "").includes("SLP_ROLE_READY")) {
        behavior.acknowledgementDelayReads = (behavior.acknowledgementDelayReads as number) - 1;
        text = `· Thinking (${behavior.acknowledgementDelayReads})\n`;
      } else {
        text = state.outputs[name] ?? "";
      }
      return {
        type: "pane_read",
        read: {
          pane_id: agent?.pane_id ?? target,
          workspace_id: agent?.workspace_id ?? "",
          tab_id: "",
          source: params.source,
          format: params.format ?? "text",
          text,
          revision: state.sequence,
          truncated: false,
        },
      };
    }
    case "pane.read": {
      const target = pane(params.pane_id);
      const agent = state.agents.find((candidate: Params) => candidate.pane_id === params.pane_id);
      return {
        type: "pane_read",
        read: {
          pane_id: params.pane_id,
          workspace_id: target?.workspace_id ?? "",
          tab_id: target?.tab_id ?? "",
          source: params.source,
          format: params.format ?? "text",
          text: state.outputs[agent?.name ?? params.pane_id] ?? "",
          revision: state.sequence,
          truncated: false,
        },
      };
    }
    case "events.subscribe": {
      subscriber.subscriptions = params.subscriptions ?? [];
      server.subscribers.add(subscriber);
      if (behavior.replayOnSubscribe !== false) {
        // Herdr 0.8.2 replays its history to every new subscriber, after the ack.
        const backlog = (state.history as FakeHerdrEvent[]).filter((event) => eventWanted(subscriber, event));
        setTimeout(() => {
          for (const event of backlog) subscriber.socket.write(`${JSON.stringify(event)}\n`);
        }, 5);
      }
      return { type: "subscription_started" };
    }
    case "plugin.list":
      return { type: "plugin_list", plugins: state.plugins };
    case "plugin.link": {
      const manifestPath = join(params.path, "herdr-plugin.toml");
      const manifest = Bun.TOML.parse(await readFile(manifestPath, "utf8")) as Params;
      const plugin = {
        plugin_id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        manifest_path: manifestPath,
        plugin_root: params.path,
        enabled: params.enabled !== false,
        min_herdr_version: manifest.min_herdr_version ?? "",
        startup: manifest.startup ?? [],
        panes: manifest.panes ?? [],
        events: manifest.events ?? [],
        actions: manifest.actions ?? [],
      };
      state.plugins = state.plugins.filter((candidate: Params) => candidate.plugin_id !== plugin.plugin_id);
      state.plugins.push(plugin);
      return { type: "plugin_linked", plugin };
    }
    case "plugin.unlink": {
      const removed = state.plugins.some((candidate: Params) => candidate.plugin_id === params.plugin_id);
      state.plugins = state.plugins.filter((candidate: Params) => candidate.plugin_id !== params.plugin_id);
      return { type: "plugin_unlinked", plugin_id: params.plugin_id, removed };
    }
    case "plugin.pane.open": {
      const anchor = params.target_pane_id ? pane(params.target_pane_id) : undefined;
      const workspaceId = params.workspace_id ?? anchor?.workspace_id;
      if (!workspaceId) throw new FakeHerdrError("invalid_params", "plugin.pane.open needs a workspace or target pane");
      const paneId = `${workspaceId}:${next("p")}`;
      const created = {
        pane_id: paneId,
        workspace_id: workspaceId,
        tab_id: anchor?.tab_id ?? `${workspaceId}:${next("t")}`,
        cwd: params.cwd ?? anchor?.cwd,
        label: `${params.plugin_id}:${params.entrypoint}`,
        plugin_id: params.plugin_id,
      };
      state.panes.push(created);
      state.processes[paneId] = {
        pane_id: paneId,
        cwd: created.cwd,
        shell_pid: 5000 + state.sequence,
        foreground_process_group_id: 6000 + state.sequence,
        foreground_processes: [{
          pid: 6000 + state.sequence,
          name: "maestro",
          command: `maestro slp ${params.entrypoint}`,
          args: ["maestro", "slp", params.entrypoint],
        }],
      };
      state.plugin_panes.push({ pane_id: paneId, entrypoint: params.entrypoint, env: params.env ?? {}, cwd: created.cwd });
      pushEvent(server, { event: "pane_created", data: { pane: created } });
      if (behavior.runtimePane === "spawn") spawnRuntimePane(server, paneId, params);
      return {
        type: "plugin_pane_opened",
        plugin_pane: { plugin_id: params.plugin_id, entrypoint: params.entrypoint, pane: created },
      };
    }
    default:
      throw new FakeHerdrError("invalid_request", `unsupported fake Herdr method: ${method}`);
  }
}

function serialize<T>(server: FakeServer, action: () => Promise<T>): Promise<T> {
  const run = server.queue.then(action, action);
  server.queue = run.catch(() => undefined);
  return run;
}

export async function installFakeHerdr(
  fixture: Fixture,
  behavior: FakeHerdrBehavior = {},
): Promise<FakeHerdrFixture> {
  const bin = join(fixture.root, "fake-herdr-bin");
  const log = join(fixture.root, "fake-herdr-log.jsonl");
  const tripwire = join(fixture.root, "fake-herdr-tripwire.jsonl");
  const socket = join(fixture.root, "herdr.sock");
  // A seat launches only through its rendered profile (A2), so the fake
  // machine carries what maestro install would have rendered into this home.
  await materializeProfiles(fixture.home, fixture.repo);
  await mkdir(bin, { recursive: true });
  await writeFile(join(bin, "herdr"), tripwireSource(tripwire));
  await chmod(join(bin, "herdr"), 0o755);
  await writeFile(log, "");
  await writeFile(tripwire, "");
  const server: FakeServer = {
    children: new Map(),
    fixture,
    listener: { stop() {} },
    logChain: Promise.resolve(),
    queue: Promise.resolve(),
    state: {
      activating: {},
      agents: [],
      behavior,
      history: [],
      helper_exits: [],
      outputs: {},
      panes: [],
      pending_workspace_closes: {},
      plugin_panes: [],
      plugins: [],
      processes: {},
      prompts: [],
      protocol: 20,
      runtime_exits: [],
      sequence: 0,
      tabs: [],
      workspaces: [],
    },
    subscribers: new Set(),
  };
  server.listener = Bun.listen<{ buffer: string; subscriber: Subscriber }>({
    unix: socket,
    socket: {
      open(connection) {
        connection.data = { buffer: "", subscriber: { socket: connection, subscriptions: [] } };
      },
      data(connection, chunk) {
        connection.data.buffer += chunk.toString();
        let index: number;
        while ((index = connection.data.buffer.indexOf("\n")) >= 0) {
          const line = connection.data.buffer.slice(0, index);
          connection.data.buffer = connection.data.buffer.slice(index + 1);
          if (line.trim() === "") continue;
          let request: { id?: string; method?: string; params?: Params };
          try {
            request = JSON.parse(line) as typeof request;
          } catch {
            connection.write(`${JSON.stringify({ id: "", error: { code: "invalid_request", message: "invalid request: not JSON" } })}\n`);
            connection.end();
            continue;
          }
          const method = String(request.method ?? "");
          const params = request.params ?? {};
          const id = request.id ?? "";
          server.logChain = server.logChain.then(() => appendFile(log, `${JSON.stringify(commandShape(method, params))}\n`));
          void serialize(server, async () => {
            await server.logChain;
            try {
              const result = await handle(server, method, params, connection.data.subscriber);
              connection.write(`${JSON.stringify({ id, result })}\n`);
            } catch (error) {
              const code = error instanceof FakeHerdrError ? error.code : "internal";
              const message = error instanceof Error ? error.message : String(error);
              connection.write(`${JSON.stringify({ id: code === "invalid_request" ? "" : id, error: { code, message } })}\n`);
            }
            if (method !== "events.subscribe") connection.end();
          });
        }
      },
      close(connection) {
        server.subscribers.delete(connection.data.subscriber);
      },
      error() {},
    },
  });
  servers.set(socket, server);
  return {
    env: {
      HERDR_SOCKET_PATH: socket,
      PATH: [bin, dirname(process.execPath), "/usr/bin", "/bin"].join(":"),
    },
    log,
    socket,
    tripwire,
  };
}

export async function fakeHerdrCommands(fake: FakeHerdrFixture): Promise<string[][]> {
  const server = servers.get(fake.socket);
  if (server) await server.logChain;
  const content = await readFile(fake.log, "utf8");
  return content.trim().length === 0
    ? []
    : content.trim().split("\n").map((line) => JSON.parse(line) as string[]);
}

// Every `herdr` CLI invocation the code under test attempted; empty proves
// the socket carried everything.
export async function tripwireInvocations(fake: FakeHerdrFixture): Promise<string[][]> {
  const content = await readFile(fake.tripwire, "utf8");
  return content.trim().length === 0
    ? []
    : content.trim().split("\n").map((line) => JSON.parse(line) as string[]);
}

export async function setFakeHerdrBehavior(
  fake: FakeHerdrFixture,
  behavior: FakeHerdrBehavior,
): Promise<void> {
  const server = requireServer(fake);
  await serialize(server, async () => {
    server.state.behavior = { ...server.state.behavior, ...behavior };
  });
}

export async function editFakeHerdrState(
  fake: FakeHerdrFixture,
  edit: (state: Record<string, any>) => void,
): Promise<void> {
  const server = requireServer(fake);
  await serialize(server, async () => {
    edit(server.state);
  });
}

export async function readFakeHerdrState(fake: FakeHerdrFixture): Promise<Record<string, any>> {
  const server = requireServer(fake);
  return serialize(server, async () => ({
    ...structuredClone(server.state),
    subscriptions: [...server.subscribers].map((subscriber) => subscriber.subscriptions),
  }));
}

// Pushes one event to every matching subscriber and returns how many got it;
// a status event also records the status on the agent in that pane.
export async function emitFakeHerdrEvent(fake: FakeHerdrFixture, event: FakeHerdrEvent): Promise<number> {
  const server = requireServer(fake);
  return serialize(server, async () => {
    if (event.event === "pane_agent_status_changed") {
      const agent = server.state.agents.find((candidate: Params) => candidate.pane_id === event.data.pane_id);
      if (agent) agent.agent_status = event.data.agent_status;
      event = {
        event: event.event,
        data: {
          agent: agent?.kind ?? null,
          display_agent: agent?.kind ?? null,
          state_labels: {},
          title: null,
          workspace_id: agent?.workspace_id ?? "",
          ...event.data,
        },
      };
    }
    return pushEvent(server, event);
  });
}

export async function waitForFakeHerdr(
  condition: () => Promise<boolean> | boolean,
  timeoutMs = 5_000,
  label = "condition",
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await condition()) return;
    await Bun.sleep(25);
  }
  throw new Error(`timed out after ${timeoutMs}ms waiting for ${label}`);
}
