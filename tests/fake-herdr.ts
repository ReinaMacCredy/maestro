import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Fixture } from "./helpers.ts";

export interface FakeHerdrBehavior {
  agents?: boolean;
  advisorRecommendation?: string | null;
  prompts?: boolean;
  roleProcesses?: boolean;
  sensor?: boolean;
  sensorDelivery?: boolean;
}

export interface FakeHerdrFixture {
  env: Record<string, string>;
  log: string;
  state: string;
}

const fakeHerdrSource = `#!/usr/bin/env bun
import { appendFile, readFile, writeFile } from "node:fs/promises";

const args = Bun.argv.slice(2);
const statePath = Bun.env.FAKE_HERDR_STATE;
const logPath = Bun.env.FAKE_HERDR_LOG;
if (!statePath || !logPath) throw new Error("fake Herdr paths are required");
await appendFile(logPath, JSON.stringify(args) + "\\n");
const state = JSON.parse(await readFile(statePath, "utf8"));
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
if (command === "workspace list") {
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
  const accepted = state.behavior.sensor !== false;
  if (accepted) {
    const target = pane(paneId);
    state.processes[paneId] = {
      pane_id: paneId,
      cwd: target?.cwd,
      shell_pid: 1000 + state.sequence,
      foreground_pgid: 2000 + state.sequence,
      foreground_processes: [{
        pid: 2000 + state.sequence,
        command: args.slice(3).join(" "),
        args: args.slice(3),
      }],
    };
  }
  await respond({ accepted, pane_id: paneId });
} else if (command === "pane close") {
  const paneId = args[2];
  state.panes = state.panes.filter((candidate) => candidate.pane_id !== paneId);
  state.agents = state.agents.filter((candidate) => candidate.pane_id !== paneId);
  delete state.processes[paneId];
  await respond({ closed: true, pane_id: paneId });
} else if (command === "pane process-info") {
  const paneId = value("--pane");
  const target = pane(paneId);
  await respond(state.processes[paneId] ?? {
    pane_id: paneId,
    cwd: target?.cwd,
    shell_pid: 1000 + state.sequence,
    foreground_pgid: null,
    foreground_processes: [],
  });
} else if (command === "agent list") {
  await respond({ type: "agent_list", agents: state.agents });
} else if (command === "agent start") {
  const name = args[2];
  const paneId = value("--pane");
  const kind = value("--kind") ?? "codex";
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
    if (state.behavior.roleProcesses !== false) {
      state.processes[paneId] = {
        pane_id: paneId,
        cwd: target?.cwd,
        shell_pid: 3000 + state.sequence,
        foreground_pgid: 4000 + state.sequence,
        foreground_processes: [{
          pid: 4000 + state.sequence,
          command: kind,
          args: [kind],
        }],
      };
    }
  }
  await respond({ accepted, name, pane_id: paneId });
} else if (command === "agent prompt") {
  const name = args[2];
  const isProbe = String(args[3] ?? "").includes("team-sensor-probe");
  const accepted = isProbe
    ? state.behavior.sensorDelivery !== false
    : state.behavior.prompts !== false;
  if (accepted) {
    const body = args[3] ?? "";
    state.prompts.push({ name, body });
    if (name.startsWith("advisor-") && body.includes("[advisor-consultation")) {
      const recommendation = state.behavior.advisorRecommendation === undefined
        ? "Use the bounded supervised path."
        : state.behavior.advisorRecommendation;
      state.outputs[name] = recommendation === null
        ? "Advisor completed without a return marker.\\n"
        : "analysis complete\\nMAESTRO_ADVISOR_RETURN " + JSON.stringify({ recommendation }) + "\\n";
      const advisor = state.agents.find((candidate) => candidate.name === name);
      if (advisor) advisor.agent_status = "done";
    }
  }
  await respond({ accepted, delivered: accepted, name });
} else if (command === "agent read") {
  const name = args[2];
  await writeFile(statePath, JSON.stringify(state, null, 2) + "\\n");
  process.stdout.write(state.outputs[name] ?? "");
} else if (command === "tab close") {
  const tabId = args[2];
  const paneIds = new Set(
    state.panes.filter((candidate) => candidate.tab_id === tabId).map((candidate) => candidate.pane_id),
  );
  state.tabs = state.tabs.filter((candidate) => candidate.tab_id !== tabId);
  state.panes = state.panes.filter((candidate) => !paneIds.has(candidate.pane_id));
  state.agents = state.agents.filter((candidate) => !paneIds.has(candidate.pane_id));
  for (const paneId of paneIds) delete state.processes[paneId];
  await respond({ closed: true, tab_id: tabId });
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
      outputs: {},
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

export async function setFakeHerdrBehavior(
  fake: FakeHerdrFixture,
  behavior: FakeHerdrBehavior,
): Promise<void> {
  const state = JSON.parse(await readFile(fake.state, "utf8")) as {
    behavior: FakeHerdrBehavior;
  };
  state.behavior = { ...state.behavior, ...behavior };
  await writeFile(fake.state, `${JSON.stringify(state, null, 2)}\n`);
}

export async function editFakeHerdrState(
  fake: FakeHerdrFixture,
  edit: (state: Record<string, unknown>) => void,
): Promise<void> {
  const state = JSON.parse(await readFile(fake.state, "utf8")) as Record<string, unknown>;
  edit(state);
  await writeFile(fake.state, `${JSON.stringify(state, null, 2)}\n`);
}

export async function readFakeHerdrState(
  fake: FakeHerdrFixture,
): Promise<Record<string, any>> {
  return JSON.parse(await readFile(fake.state, "utf8")) as Record<string, any>;
}
