import { join } from "node:path";
import { CliError } from "../kernel/cli.ts";
import { resolveHomeDirectory } from "./home.ts";

// Hub d96: SLP reaches Herdr over its socket API, newline JSON over
// HERDR_SOCKET_PATH, one request per connection, events.subscribe kept open.
// The typed surface below is the whole set SLP uses; agent.wait, agent.focus,
// pane.send_text and notification.show stay out (d767, d753, advisor F20).

export const herdrProtocol = 20;

interface SlpRuntimeEvidence {
  code?: string;
  directory?: string;
  harness?: string;
  herdrCode?: string;
  paneTail?: readonly string[];
}

export class SlpRuntimeError extends CliError {
  readonly herdrCode: string | null;

  constructor(
    message: string,
    readonly command: readonly string[],
    readonly stderr?: string,
    evidence: SlpRuntimeEvidence = {},
  ) {
    super(evidence.code ?? "SLP_RUNTIME", message, {
      command: [...command],
      ...(stderr ? { stderr } : {}),
      ...(evidence.harness ? { harness: evidence.harness } : {}),
      ...(evidence.directory ? { directory: evidence.directory } : {}),
      ...(evidence.paneTail ? { paneTail: [...evidence.paneTail] } : {}),
      ...(evidence.herdrCode ? { herdrCode: evidence.herdrCode } : {}),
    });
    this.herdrCode = evidence.herdrCode ?? null;
  }
}

export type HerdrAgentStatus = "blocked" | "done" | "idle" | "unknown" | "working";

export interface HerdrWorkspace {
  cwd?: string | null;
  label?: string;
  workspace_id: string;
}

export interface HerdrTab {
  label?: string;
  root_pane_id?: string;
  tab_id: string;
  workspace_id: string;
}

export interface HerdrPane {
  cwd?: string | null;
  pane_id: string;
  tab_id?: string;
  workspace_id: string;
}

export interface HerdrAgent {
  agent?: string | null;
  // The harness session the SessionStart hook reported for the pane; a new
  // value proves a new conversation (Claude Code /clear, live 2026-09-05).
  agent_session?: { value?: string | null } | null;
  agent_status: HerdrAgentStatus | string;
  interactive_ready?: boolean;
  launch_pending?: boolean;
  name?: string | null;
  pane_id: string;
  workspace_id: string;
}

export interface HerdrProcessInfo {
  foreground_process_group_id?: number | null;
  foreground_processes?: Array<{ argv?: string[] | null; name?: string; pid: number }>;
  pane_id: string;
  shell_pid?: number | null;
}

export type HerdrReadSource = "detection" | "recent" | "recent_unwrapped" | "visible";

export type HerdrSubscription =
  | { agent_status?: HerdrAgentStatus; pane_id: string; type: "pane.agent_status_changed" }
  | { type: "pane.agent_detected" | "pane.closed" | "pane.created" | "pane.exited" };

export interface HerdrEvent {
  data: Record<string, unknown> & { pane_id?: string; workspace_id?: string };
  event: string;
}

export interface HerdrEventStream {
  close(): void;
  events: AsyncIterable<HerdrEvent>;
}

interface HerdrResponse {
  error?: { code: string; message: string };
  id: string;
  result?: Record<string, unknown>;
}

export function herdrSocketPath(environment: Record<string, string | undefined> = process.env): string {
  if (environment.HERDR_SOCKET_PATH) return environment.HERDR_SOCKET_PATH;
  const home = resolveHomeDirectory({ environmentHome: environment.HOME });
  return join(home, ".config", "herdr", "herdr.sock");
}

function connectionFailure(socketPath: string, method: string, error: unknown): SlpRuntimeError {
  const message = error instanceof Error ? error.message : String(error);
  return new SlpRuntimeError(
    `cannot reach Herdr at ${socketPath}: ${message}`,
    [method],
    undefined,
    { code: "HERDR_UNAVAILABLE" },
  );
}

function responseFailure(method: string, error: { code: string; message: string }): SlpRuntimeError {
  // Advisor F19: a protocol bump only fails the call whose method the running
  // Herdr does not know, and that failure names the method.
  if (error.code === "invalid_request" && error.message.includes(`unknown variant \`${method}\``)) {
    return new SlpRuntimeError(
      `Herdr does not know ${method}; the running Herdr is older or newer than this maestro expects (protocol ${herdrProtocol})`,
      [method],
      undefined,
      { code: "HERDR_METHOD_MISSING", herdrCode: error.code },
    );
  }
  return new SlpRuntimeError(
    `Herdr ${method} failed: ${error.code}: ${error.message}`,
    [method],
    undefined,
    { herdrCode: error.code },
  );
}

class LineReader {
  private buffer = "";

  push(chunk: Uint8Array | string, onLine: (line: string) => void): void {
    this.buffer += typeof chunk === "string" ? chunk : new TextDecoder().decode(chunk);
    let index: number;
    while ((index = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, index);
      this.buffer = this.buffer.slice(index + 1);
      if (line.trim() !== "") onLine(line);
    }
  }
}

export class HerdrClient {
  readonly socketPath: string;
  private protocolChecked = false;

  constructor(
    private readonly environment: Record<string, string | undefined> = process.env,
    private readonly timeoutMs = 15_000,
  ) {
    this.socketPath = herdrSocketPath(environment);
  }

  private async connect(
    method: string,
    onLine: (line: string) => void,
    onClose: () => void,
  ): Promise<{ end(): void; write(data: string): void }> {
    const reader = new LineReader();
    try {
      const socket = await Bun.connect({
        unix: this.socketPath,
        socket: {
          data(_socket, chunk) {
            reader.push(chunk, onLine);
          },
          close() {
            onClose();
          },
          error() {
            onClose();
          },
          connectError() {
            onClose();
          },
        },
      });
      return {
        end: () => socket.end(),
        write: (data: string) => {
          socket.write(data);
        },
      };
    } catch (error) {
      throw connectionFailure(this.socketPath, method, error);
    }
  }

  private async rawRequest(method: string, params: object, timeoutMs: number): Promise<Record<string, unknown>> {
    const id = `${process.pid}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    let settle: (value: HerdrResponse | null) => void = () => {};
    const response = new Promise<HerdrResponse | null>((resolve) => {
      settle = resolve;
    });
    const connection = await this.connect(
      method,
      (line) => {
        try {
          const parsed = JSON.parse(line) as HerdrResponse;
          // Herdr answers a request it could not parse with an empty id.
          if (parsed.id === id || parsed.id === "" || parsed.error) settle(parsed);
        } catch {
          settle({ id, error: { code: "invalid_response", message: `Herdr returned invalid JSON for ${method}` } });
        }
      },
      () => settle(null),
    );
    connection.write(`${JSON.stringify({ id, method, params })}\n`);
    let timer: ReturnType<typeof setTimeout> | null = null;
    const timeout = new Promise<"timeout">((resolve) => {
      timer = setTimeout(() => resolve("timeout"), timeoutMs);
    });
    const outcome = await Promise.race([response, timeout]);
    if (timer) clearTimeout(timer);
    connection.end();
    if (outcome === "timeout") {
      throw new SlpRuntimeError(`Herdr command timed out after ${timeoutMs}ms: ${method}`, [method]);
    }
    if (outcome === null) {
      throw new SlpRuntimeError(`Herdr closed the connection without answering ${method}`, [method]);
    }
    if (outcome.error) throw responseFailure(method, outcome.error);
    return outcome.result ?? {};
  }

  // The protocol is compared once per client; a mismatch warns with both
  // numbers and the calls go on (advisor F19).
  private async checkProtocol(timeoutMs: number): Promise<void> {
    if (this.protocolChecked) return;
    this.protocolChecked = true;
    const pong = await this.rawRequest("ping", {}, timeoutMs);
    const protocol = typeof pong.protocol === "number" ? pong.protocol : null;
    if (protocol !== null && protocol !== herdrProtocol) {
      process.stderr.write(
        `warning: Herdr speaks protocol ${protocol} (version ${String(pong.version ?? "unknown")}) while maestro was built for protocol ${herdrProtocol}; a call fails only if Herdr lacks its method\n`,
      );
    }
  }

  async request<T extends Record<string, unknown> = Record<string, unknown>>(
    method: string,
    params: object = {},
    timeoutMs = this.timeoutMs,
  ): Promise<T> {
    await this.checkProtocol(timeoutMs);
    return await this.rawRequest(method, params, timeoutMs) as T;
  }

  async subscribe(subscriptions: HerdrSubscription[]): Promise<HerdrEventStream> {
    await this.checkProtocol(this.timeoutMs);
    const method = "events.subscribe";
    const id = `${process.pid}-subscribe-${Date.now().toString(36)}`;
    const queue: HerdrEvent[] = [];
    let waiting: ((value: IteratorResult<HerdrEvent>) => void) | null = null;
    let closed = false;
    let acknowledged: ((response: HerdrResponse | null) => void) | null = null;
    const acknowledgement = new Promise<HerdrResponse | null>((resolve) => {
      acknowledged = resolve;
    });
    const finish = () => {
      if (closed) return;
      closed = true;
      acknowledged?.(null);
      if (waiting) {
        waiting({ done: true, value: undefined });
        waiting = null;
      }
    };
    const connection = await this.connect(
      method,
      (line) => {
        let parsed: HerdrResponse & Partial<HerdrEvent>;
        try {
          parsed = JSON.parse(line) as typeof parsed;
        } catch {
          return;
        }
        if (acknowledged) {
          const settle = acknowledged;
          acknowledged = null;
          settle(parsed);
          return;
        }
        if (typeof parsed.event !== "string") return;
        // Herdr 0.8.2 names a live push by its subscription (pane.agent_status_changed)
        // and a replayed one by the schema kind (pane_agent_status_changed); every
        // consumer sees the schema kind.
        const event: HerdrEvent = { data: (parsed.data ?? {}) as HerdrEvent["data"], event: parsed.event.replaceAll(".", "_") };
        if (waiting) {
          const resolve = waiting;
          waiting = null;
          resolve({ done: false, value: event });
        } else {
          queue.push(event);
        }
      },
      finish,
    );
    connection.write(`${JSON.stringify({ id, method, params: { subscriptions } })}\n`);
    const timer = setTimeout(() => acknowledged?.(null), this.timeoutMs);
    const first = await acknowledgement;
    clearTimeout(timer);
    if (!first) {
      connection.end();
      throw new SlpRuntimeError(`Herdr did not acknowledge ${method}`, [method]);
    }
    if (first.error) {
      connection.end();
      throw responseFailure(method, first.error);
    }
    const events: AsyncIterable<HerdrEvent> = {
      [Symbol.asyncIterator]() {
        return {
          next(): Promise<IteratorResult<HerdrEvent>> {
            const queued = queue.shift();
            if (queued) return Promise.resolve({ done: false, value: queued });
            if (closed) return Promise.resolve({ done: true, value: undefined });
            return new Promise((resolve) => {
              waiting = resolve;
            });
          },
          return(): Promise<IteratorResult<HerdrEvent>> {
            connection.end();
            finish();
            return Promise.resolve({ done: true, value: undefined });
          },
        };
      },
    };
    return {
      close() {
        connection.end();
        finish();
      },
      events,
    };
  }

  async workspaceList(timeoutMs?: number): Promise<HerdrWorkspace[]> {
    const result = await this.request<{ workspaces?: HerdrWorkspace[] }>("workspace.list", {}, timeoutMs);
    return Array.isArray(result.workspaces) ? result.workspaces : [];
  }

  async workspaceCreate(params: { cwd: string; label: string }): Promise<{ root_pane?: HerdrPane; workspace?: HerdrWorkspace }> {
    return this.request("workspace.create", { ...params, focus: false });
  }

  async workspaceClose(workspaceId: string): Promise<void> {
    await this.request("workspace.close", { workspace_id: workspaceId });
  }

  async tabList(workspaceId: string): Promise<HerdrTab[]> {
    const result = await this.request<{ tabs?: HerdrTab[] }>("tab.list", { workspace_id: workspaceId });
    return Array.isArray(result.tabs) ? result.tabs : [];
  }

  async tabCreate(params: { cwd: string; label: string; workspace_id: string }): Promise<{ root_pane?: HerdrPane; tab?: HerdrTab }> {
    return this.request("tab.create", { ...params, focus: false });
  }

  async tabClose(tabId: string): Promise<void> {
    await this.request("tab.close", { tab_id: tabId });
  }

  async paneList(workspaceId: string, timeoutMs?: number): Promise<HerdrPane[]> {
    const result = await this.request<{ panes?: HerdrPane[] }>("pane.list", { workspace_id: workspaceId }, timeoutMs);
    return Array.isArray(result.panes) ? result.panes : [];
  }

  async paneGet(paneId: string): Promise<HerdrPane | null> {
    const result = await this.request<{ pane?: HerdrPane }>("pane.get", { pane_id: paneId });
    return result.pane ?? null;
  }

  async paneRead(paneId: string, source: HerdrReadSource, lines: number): Promise<string> {
    const result = await this.request<{ read?: { text?: string } }>("pane.read", {
      format: "text",
      lines,
      pane_id: paneId,
      source,
    });
    return typeof result.read?.text === "string" ? result.read.text : "";
  }

  async paneProcessInfo(paneId: string, timeoutMs?: number): Promise<HerdrProcessInfo> {
    const result = await this.request<{ process_info?: HerdrProcessInfo }>(
      "pane.process_info",
      { pane_id: paneId },
      timeoutMs,
    );
    return result.process_info ?? { pane_id: paneId };
  }

  async paneClose(paneId: string): Promise<void> {
    await this.request("pane.close", { pane_id: paneId });
  }

  // d830: the socket form of `herdr pane run` on a shell pane; never an agent pane.
  async paneSendInput(paneId: string, text: string): Promise<void> {
    await this.request("pane.send_input", { keys: ["enter"], pane_id: paneId, text });
  }

  // The socket forms of `herdr pane send-text` and `herdr pane send-keys`: a
  // harness slash command lands as its own input and the enter key as another
  // (live g22 2026-09-05: one input reached Claude Code as "/clearslp ...").
  async paneSendText(paneId: string, text: string): Promise<void> {
    await this.request("pane.send_input", { pane_id: paneId, text });
  }

  async paneSendKeys(paneId: string, keys: string[]): Promise<void> {
    await this.request("pane.send_input", { keys, pane_id: paneId });
  }

  async agentList(timeoutMs?: number): Promise<HerdrAgent[]> {
    const result = await this.request<{ agents?: HerdrAgent[] }>("agent.list", {}, timeoutMs);
    return Array.isArray(result.agents) ? result.agents : [];
  }

  async agentGet(target: string): Promise<HerdrAgent | null> {
    const result = await this.request<{ agent?: HerdrAgent }>("agent.get", { target });
    return result.agent ?? null;
  }

  async agentRead(target: string, source: HerdrReadSource, lines: number): Promise<string> {
    const result = await this.request<{ read?: { text?: string } }>("agent.read", {
      format: "text",
      lines,
      source,
      target,
    });
    return typeof result.read?.text === "string" ? result.read.text : "";
  }

  async agentStart(
    params: { args: string[]; kind: string; name: string; pane_id: string; timeout_ms: number },
    timeoutMs?: number,
  ): Promise<HerdrAgent | null> {
    const result = await this.request<{ agent?: HerdrAgent }>("agent.start", params, timeoutMs);
    return result.agent ?? null;
  }

  async agentPrompt(
    target: string,
    text: string,
    wait?: { timeout_ms: number },
    timeoutMs?: number,
  ): Promise<HerdrAgent | null> {
    const result = await this.request<{ agent?: HerdrAgent }>(
      "agent.prompt",
      wait ? { target, text, wait } : { target, text },
      timeoutMs,
    );
    return result.agent ?? null;
  }

  async pluginList(): Promise<Array<{ manifest_path?: string; plugin_id: string; plugin_root?: string }>> {
    const result = await this.request<{ plugins?: Array<{ manifest_path?: string; plugin_id: string; plugin_root?: string }> }>(
      "plugin.list",
      {},
    );
    return Array.isArray(result.plugins) ? result.plugins : [];
  }

  async pluginLink(path: string): Promise<void> {
    await this.request("plugin.link", { path });
  }

  async pluginUnlink(pluginId: string): Promise<boolean> {
    const result = await this.request<{ removed?: boolean }>("plugin.unlink", { plugin_id: pluginId });
    return result.removed === true;
  }

  async pluginPaneOpen(params: {
    cwd: string;
    entrypoint: string;
    env: Record<string, string>;
    placement: "split" | "tab";
    plugin_id: string;
    target_pane_id?: string;
    workspace_id?: string;
  }): Promise<HerdrPane | null> {
    const result = await this.request<{ plugin_pane?: { pane?: HerdrPane } }>("plugin.pane.open", {
      ...params,
      ...(params.placement === "split" ? { direction: "right" } : {}),
      focus: false,
    });
    return result.plugin_pane?.pane ?? null;
  }
}
