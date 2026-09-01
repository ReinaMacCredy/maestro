import { Database } from "bun:sqlite";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Fixture } from "./helpers.ts";

export interface FakeHerdrBehavior {
  acknowledgementPrefixes?: Partial<Record<"team-supervisor" | "lead" | "peer", string>>;
  agentBusyAttempts?: number;
  agentStartDelayMs?: number;
  agents?: boolean;
  closeResources?: boolean;
  closeWorkspaceWithLastTab?: boolean;
  closeWorkspace?: boolean;
  codexNotReadyAttempts?: number;
  failWorkspaceId?: string;
  invalidAcknowledgementField?: "challenge" | "pack";
  paneRunEmptyOutput?: boolean;
  processInfo?: boolean;
  processInfoDelayMs?: number;
  promptStalledAttempts?: number;
  prompts?: boolean;
  settleAgents?: boolean;
  wrapAcknowledgements?: boolean;
  workspaceCloseListLag?: number;
  workspaceListDelayMs?: number;
}

export interface FakeHerdrFixture {
  env: Record<string, string>;
  log: string;
  state: string;
}

const fakeHerdrSource = `#!/usr/bin/env bun
import { appendFile, readFile, writeFile } from "node:fs/promises";
import { Database } from "bun:sqlite";

const args = Bun.argv.slice(2);
const statePath = Bun.env.FAKE_HERDR_STATE;
const logPath = Bun.env.FAKE_HERDR_LOG;
if (!statePath || !logPath) throw new Error("fake Herdr paths are required");
const stateLock = new Database(statePath + ".lock.sqlite");
stateLock.exec("PRAGMA busy_timeout = 300000");
stateLock.exec("CREATE TABLE IF NOT EXISTS state_lock (id INTEGER PRIMARY KEY)");
let stateLockHeld = false;
const acquireStateLock = () => {
  if (stateLockHeld) return;
  stateLock.exec("BEGIN IMMEDIATE");
  stateLockHeld = true;
};
const releaseStateLock = () => {
  if (!stateLockHeld) return;
  stateLock.exec("COMMIT");
  stateLockHeld = false;
};
process.on("exit", () => {
  try { releaseStateLock(); } catch {}
  try { stateLock.close(); } catch {}
});
acquireStateLock();
await appendFile(logPath, JSON.stringify(args) + "\\n");
let state = JSON.parse(await readFile(statePath, "utf8"));
const value = (name) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
};
const next = (prefix) => prefix + String(++state.sequence);
const pane = (paneId) => state.panes.find((candidate) => candidate.pane_id === paneId);
const respond = async (result) => {
  await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
  process.stdout.write(JSON.stringify({ id: "fake", result }) + "\\n");
};

const command = args.slice(0, 2).join(" ");
const receiptDatabasePath = Bun.env.FAKE_HERDR_RECEIPT_DB;
const receiptOperation = Bun.env.FAKE_HERDR_RECEIPT_OPERATION;
const receiptTeam = Bun.env.FAKE_HERDR_RECEIPT_TEAM;
if (receiptDatabasePath && receiptOperation && receiptTeam) {
  const database = new Database(receiptDatabasePath, { create: false, readonly: true, strict: true });
  const receipt = database
    .query("SELECT status FROM team_receipts WHERE operation_id = ?")
    .get(receiptOperation);
  const snapshot = database
    .query("SELECT COUNT(*) AS count FROM team_lifecycle WHERE team_id = ?")
    .get(receiptTeam);
  database.close();
  const audit = {
    command,
    receiptStatus: receipt?.status ?? null,
    snapshotCount: snapshot?.count ?? 0,
  };
  state.receipt_audit.push(audit);
  if (audit.receiptStatus !== "ATTEMPTED" || audit.snapshotCount !== 0) {
    await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    process.stderr.write("runtime command ran outside ATTEMPTED receipt: " + JSON.stringify(audit) + "\\n");
    process.exit(65);
  }
}
if (command === "workspace list") {
  if (state.behavior.workspaceListDelayMs) await Bun.sleep(state.behavior.workspaceListDelayMs);
  for (const [workspaceId, remaining] of Object.entries(state.pending_workspace_closes)) {
    if (remaining <= 0) {
      state.workspaces = state.workspaces.filter(
        (candidate) => candidate.workspace_id !== workspaceId,
      );
      delete state.pending_workspace_closes[workspaceId];
    } else {
      state.pending_workspace_closes[workspaceId] = remaining - 1;
    }
  }
  await respond({ type: "workspace_list", workspaces: state.workspaces });
} else if (command === "workspace create") {
  const workspaceId = next("w");
  const paneId = workspaceId + ":" + next("p");
  const cwd = value("--cwd");
  const label = value("--label");
  const workspace = { workspace_id: workspaceId, cwd, label };
  const rootPane = { pane_id: paneId, workspace_id: workspaceId, cwd, label };
  state.workspaces.push(workspace);
  state.panes.push(rootPane);
  await respond({ workspace, root_pane: rootPane });
} else if (command === "workspace close") {
  const workspaceId = args[2];
  if (!state.workspaces.some((candidate) => candidate.workspace_id === workspaceId)) {
    process.stderr.write(JSON.stringify({
      id: "cli:workspace:close",
      error: { code: "workspace_not_found", message: "workspace not found: " + workspaceId },
    }) + "\\n");
    process.exit(1);
  }
  const closed =
    state.behavior.closeResources !== false &&
    state.behavior.closeWorkspace !== false &&
    state.behavior.failWorkspaceId !== workspaceId;
  if (closed) {
    const paneIds = new Set(
      state.panes
        .filter((candidate) => candidate.workspace_id === workspaceId)
        .map((candidate) => candidate.pane_id),
    );
    if ((state.behavior.workspaceCloseListLag ?? 0) > 0) {
      state.pending_workspace_closes[workspaceId] = state.behavior.workspaceCloseListLag;
      state.behavior.workspaceCloseListLag = 0;
    } else {
      state.workspaces = state.workspaces.filter(
        (candidate) => candidate.workspace_id !== workspaceId,
      );
    }
    state.tabs = state.tabs.filter((candidate) => candidate.workspace_id !== workspaceId);
    state.panes = state.panes.filter((candidate) => candidate.workspace_id !== workspaceId);
    state.agents = state.agents.filter((candidate) => candidate.workspace_id !== workspaceId);
    for (const paneId of paneIds) delete state.processes[paneId];
  }
  await respond({ closed, workspace_id: workspaceId });
} else if (command === "tab list") {
  const workspaceId = value("--workspace");
  await respond({
    type: "tab_list",
    tabs: state.tabs.filter((candidate) => !workspaceId || candidate.workspace_id === workspaceId),
  });
} else if (command === "tab create") {
  const workspaceId = value("--workspace");
  const tabId = workspaceId + ":" + next("t");
  const paneId = tabId + ":" + next("p");
  const cwd = value("--cwd");
  const label = value("--label");
  const tab = { tab_id: tabId, workspace_id: workspaceId, root_pane_id: paneId, label };
  const rootPane = { pane_id: paneId, workspace_id: workspaceId, tab_id: tabId, cwd, label };
  state.tabs.push(tab);
  state.panes.push(rootPane);
  await respond({ tab, root_pane: rootPane });
} else if (command === "pane list") {
  const workspaceId = value("--workspace");
  await respond({
    type: "pane_list",
    panes: state.panes.filter((candidate) => !workspaceId || candidate.workspace_id === workspaceId),
  });
} else if (command === "pane split") {
  const source = pane(value("--pane"));
  const paneId = source.workspace_id + ":" + next("p");
  const created = {
    pane_id: paneId,
    workspace_id: source.workspace_id,
    tab_id: source.tab_id,
    cwd: value("--cwd") ?? source.cwd,
  };
  state.panes.push(created);
  await respond({ pane: created });
} else if (command === "pane run") {
  const paneId = args[2];
  const target = pane(paneId);
  const runArgs = args.slice(3);
  const stopGrant = runArgs.some((arg) => arg.startsWith("MAESTRO_SLP_STOP_GRANT="));
  if (runArgs[0] === "/usr/bin/env" && stopGrant) {
    const extraEnvironment = {};
    let commandIndex = 1;
    while (commandIndex < runArgs.length && runArgs[commandIndex].includes("=")) {
      const assignment = runArgs[commandIndex];
      const separator = assignment.indexOf("=");
      extraEnvironment[assignment.slice(0, separator)] = assignment.slice(separator + 1);
      commandIndex += 1;
    }
    const childCommand = runArgs.slice(commandIndex);
    await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    releaseStateLock();
    const child = Bun.spawn(childCommand, {
      cwd: target?.cwd,
      env: {
        ...process.env,
        ...extraEnvironment,
        HERDR_PANE_ID: paneId,
        HERDR_WORKSPACE_ID: target?.workspace_id,
      },
      stderr: "pipe",
      stdout: "pipe",
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    acquireStateLock();
    state = JSON.parse(await readFile(statePath, "utf8"));
    if (exitCode !== 0) {
      process.stderr.write(stderr || stdout || "stop helper failed\\n");
      process.exit(exitCode);
    }
    if (state.behavior.paneRunEmptyOutput) {
      await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    } else {
      await respond({ accepted: true, pane_id: paneId });
    }
  } else {
    state.processes[paneId] = {
      pane_id: paneId,
      cwd: target?.cwd,
      shell_pid: 1000 + state.sequence,
      foreground_process_group_id: 2000 + state.sequence,
      foreground_processes: [{
        pid: 2000 + state.sequence,
        command: runArgs.join(" "),
        args: runArgs,
      }],
    };
    if (state.behavior.paneRunEmptyOutput) {
      await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    } else {
      await respond({ accepted: true, pane_id: paneId });
    }
  }
} else if (command === "pane close") {
  const paneId = args[2];
  const closed = state.behavior.closeResources !== false;
  if (closed) {
    const target = pane(paneId);
    state.panes = state.panes.filter((candidate) => candidate.pane_id !== paneId);
    state.agents = state.agents.filter((candidate) => candidate.pane_id !== paneId);
    delete state.processes[paneId];
    if (target?.tab_id) {
      const remaining = state.panes.filter((candidate) => candidate.tab_id === target.tab_id);
      const tab = state.tabs.find((candidate) => candidate.tab_id === target.tab_id);
      if (remaining.length === 0) {
        state.tabs = state.tabs.filter((candidate) => candidate.tab_id !== target.tab_id);
      } else if (tab?.root_pane_id === paneId) {
        tab.root_pane_id = remaining[0].pane_id;
      }
    }
  }
  await respond({ closed, pane_id: paneId });
} else if (command === "pane process-info") {
  if (state.behavior.processInfoDelayMs) await Bun.sleep(state.behavior.processInfoDelayMs);
  if (state.behavior.processInfo === false) {
    process.stderr.write("injected process-info failure\\n");
    process.exit(66);
  }
  const paneId = value("--pane");
  const target = pane(paneId);
  await respond({
    type: "pane_process_info",
    process_info: state.processes[paneId] ?? {
      pane_id: paneId,
      cwd: target?.cwd,
      shell_pid: 1000 + state.sequence,
      foreground_process_group_id: null,
      foreground_processes: [],
    },
  });
} else if (command === "agent list") {
  await respond({ type: "agent_list", agents: state.agents });
} else if (command === "agent start") {
  const name = args[2];
  const paneId = value("--pane");
  const kind = value("--kind") ?? "codex";
  if (state.behavior.agentStartDelayMs) await Bun.sleep(state.behavior.agentStartDelayMs);
  if ((state.behavior.agentBusyAttempts ?? 0) > 0) {
    state.behavior.agentBusyAttempts -= 1;
    await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    process.stderr.write(JSON.stringify({
      id: "cli:agent:start",
      error: {
        code: "agent_pane_busy",
        message: "agent target pane " + paneId + " is not an available shell",
      },
    }) + "\\n");
    process.exit(1);
  }
  const accepted = state.behavior.agents !== false;
  if (accepted && !state.agents.some((candidate) => candidate.name === name)) {
    const target = pane(paneId);
    state.agents.push({
      name,
      pane_id: paneId,
      workspace_id: target?.workspace_id,
      agent_status: "working",
      kind,
    });
    state.processes[paneId] = {
      pane_id: paneId,
      cwd: target?.cwd,
      shell_pid: 3000 + state.sequence,
      foreground_process_group_id: 4000 + state.sequence,
      foreground_processes: [{
        pid: 4000 + state.sequence,
        command: kind,
        args: [kind],
      }],
    };
  }
  if (kind === "codex" && (state.behavior.codexNotReadyAttempts ?? 0) > 0) {
    state.behavior.codexNotReadyAttempts -= 1;
    const agent = state.agents.find((candidate) => candidate.name === name);
    if (agent) agent.agent_status = "idle";
    await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    process.stderr.write(JSON.stringify({
      id: "cli:agent:start",
      error: {
        code: "agent_not_ready",
        message: "agent " + name + " is blocked during startup and is not ready for prompts",
      },
    }) + "\\n");
    process.exit(1);
  }
  await respond({ accepted, name, pane_id: paneId });
} else if (command === "agent prompt") {
  const name = args[2];
  if ((state.behavior.promptStalledAttempts ?? 0) > 0) {
    state.behavior.promptStalledAttempts -= 1;
    await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
    process.stderr.write(JSON.stringify({
      id: "cli:agent:prompt",
      error: {
        code: "agent_prompt_stalled",
        message: "agent prompt produced no observed state change within 5000 ms",
      },
    }) + "\\n");
    process.exit(1);
  }
  const accepted = state.behavior.prompts !== false;
  if (accepted) {
    const body = args[3] ?? "";
    state.prompts.push({ name, body });
    const field = (label) => {
      const match = new RegExp("^" + label + ": (.+)$", "m").exec(body);
      return match?.[1]?.trim();
    };
    const team = field("Team");
    const generation = field("Generation");
    const role = field("Role");
    const instance = field("Role instance");
    const pack = field("Pack SHA-256");
    const brief = field("Brief SHA-256");
    const challengeLeft = field("Challenge left");
    const challengeRight = field("Challenge right");
    if (
      team && generation && role && instance && pack && brief &&
      challengeLeft && challengeRight
    ) {
      const acknowledgement = [
        "SLP_ROLE_READY",
        "team=" + team,
        "generation=" + generation,
        "role=" + role,
        "instance=" + instance,
        "pack=" + pack,
        "brief=" + brief,
        "challenge=" + challengeLeft + challengeRight,
      ];
      if (state.behavior.invalidAcknowledgementField === "pack") {
        acknowledgement[5] = "pack=" + "0".repeat(64);
      }
      if (state.behavior.invalidAcknowledgementField === "challenge") {
        acknowledgement[7] = "challenge=" + "0".repeat(32);
      }
      const acknowledgementLines = state.behavior.wrapAcknowledgements
        ? [
          acknowledgement.slice(0, 5).join(" "),
          acknowledgement.slice(5, 7).join(" "),
          acknowledgement.slice(7).join(" "),
        ]
        : [acknowledgement.join(" ")];
      state.outputs[name] =
        (state.behavior.acknowledgementPrefixes?.[role] ?? "") +
        acknowledgementLines.join("\\n") + "\\n";
    }
    if (state.behavior.settleAgents !== false) {
      const agent = state.agents.find((candidate) => candidate.name === name);
      if (agent) agent.agent_status = "idle";
    }
  }
  await respond({ accepted, delivered: accepted, name });
} else if (command === "agent read") {
  const name = args[2];
  await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
  process.stdout.write(state.outputs[name] ?? "");
} else if (command === "tab close") {
  const tabId = args[2];
  const targetTab = state.tabs.find((candidate) => candidate.tab_id === tabId);
  const closed = state.behavior.closeResources !== false;
  const paneIds = new Set(
    state.panes.filter((candidate) => candidate.tab_id === tabId).map((candidate) => candidate.pane_id),
  );
  if (closed) {
    state.tabs = state.tabs.filter((candidate) => candidate.tab_id !== tabId);
    state.panes = state.panes.filter((candidate) => !paneIds.has(candidate.pane_id));
    state.agents = state.agents.filter((candidate) => !paneIds.has(candidate.pane_id));
    for (const paneId of paneIds) delete state.processes[paneId];
    if (
      state.behavior.closeWorkspaceWithLastTab &&
      targetTab?.workspace_id &&
      !state.tabs.some((candidate) => candidate.workspace_id === targetTab.workspace_id)
    ) {
      state.workspaces = state.workspaces.filter(
        (candidate) => candidate.workspace_id !== targetTab.workspace_id,
      );
    }
  }
  await respond({ closed, tab_id: tabId });
} else {
  process.stderr.write("unsupported fake Herdr command: " + args.join(" ") + "\\n");
  process.exit(64);
}
`;

export async function installFakeHerdr(
  fixture: Fixture,
  behavior: FakeHerdrBehavior = {},
): Promise<FakeHerdrFixture> {
  const bin = join(fixture.root, "fake-herdr-bin");
  const state = join(fixture.root, "fake-herdr-state.json");
  const log = join(fixture.root, "fake-herdr-log.jsonl");
  await mkdir(bin, { recursive: true });
  await writeFile(join(bin, "herdr"), fakeHerdrSource);
  await chmod(join(bin, "herdr"), 0o755);
  await writeFile(
    state,
    `${JSON.stringify({
      agents: [],
      behavior,
      panes: [],
      processes: {},
      prompts: [],
      receipt_audit: [],
      outputs: {},
      pending_workspace_closes: {},
      sequence: 0,
      tabs: [],
      workspaces: [],
    }, null, 2)}\n`,
  );
  await writeFile(log, "");
  return {
    env: {
      FAKE_HERDR_LOG: log,
      FAKE_HERDR_STATE: state,
      PATH: [bin, dirname(process.execPath), "/usr/bin", "/bin"].join(":"),
    },
    log,
    state,
  };
}

export async function fakeHerdrCommands(fake: FakeHerdrFixture): Promise<string[][]> {
  const content = await readFile(fake.log, "utf8");
  return content.trim().length === 0
    ? []
    : content.trim().split("\n").map((line) => JSON.parse(line) as string[]);
}

async function withFakeHerdrStateLock<T>(
  fake: FakeHerdrFixture,
  action: () => Promise<T>,
): Promise<T> {
  const lock = new Database(`${fake.state}.lock.sqlite`);
  lock.exec("PRAGMA busy_timeout = 300000");
  lock.exec("CREATE TABLE IF NOT EXISTS state_lock (id INTEGER PRIMARY KEY)");
  lock.exec("BEGIN IMMEDIATE");
  try {
    const result = await action();
    lock.exec("COMMIT");
    return result;
  } catch (error) {
    try {
      lock.exec("ROLLBACK");
    } catch {}
    throw error;
  } finally {
    lock.close();
  }
}

export async function setFakeHerdrBehavior(
  fake: FakeHerdrFixture,
  behavior: FakeHerdrBehavior,
): Promise<void> {
  await withFakeHerdrStateLock(fake, async () => {
    const state = JSON.parse(await readFile(fake.state, "utf8")) as {
      behavior: FakeHerdrBehavior;
    };
    state.behavior = { ...state.behavior, ...behavior };
    await writeFile(fake.state, `${JSON.stringify(state, null, 2)}\n`);
  });
}

export async function editFakeHerdrState(
  fake: FakeHerdrFixture,
  edit: (state: Record<string, unknown>) => void,
): Promise<void> {
  await withFakeHerdrStateLock(fake, async () => {
    const state = JSON.parse(await readFile(fake.state, "utf8")) as Record<string, unknown>;
    edit(state);
    await writeFile(fake.state, `${JSON.stringify(state, null, 2)}\n`);
  });
}

export async function readFakeHerdrState(
  fake: FakeHerdrFixture,
): Promise<Record<string, any>> {
  return withFakeHerdrStateLock(
    fake,
    async () => JSON.parse(await readFile(fake.state, "utf8")) as Record<string, any>,
  );
}
