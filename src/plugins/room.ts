import type { Database } from "bun:sqlite";
import { chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

export const installInRoomMessage =
  "~/maestro is the Hub, not a repository; run maestro install from a repository checkout, which maintains the Hub";

export function isRoom(database: Database): boolean {
  const table = database
    .query<{ present: number }, []>(
      "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = 'meta'",
    )
    .get();
  if (!table) return false;
  return database
    .query<{ value: string }, [string]>("SELECT value FROM meta WHERE key = ?")
    .get("kind")?.value === "room";
}

const agents = `# Maestro Hub

Read \`OWNER.md\`, then read \`SLP.md\`. This workspace is the Hub Supervisor.
Use only the shared section and Hub Supervisor section of \`SLP.md\` for SLP
teams. The project snapshot copied by \`maestro team start\` is the pinned team
contract; do not maintain separate role or skill files.
`;

const identity = `# IDENTITY — Hub Supervisor

The canonical SLP contract is \`SLP.md\`. Apply its shared and Hub Supervisor
sections. Communicate with each team only through its Team Supervisor. Owner
approval remains required for external effects unless \`OWNER.md\` explicitly
delegates that authority.
`;

const owner = `# OWNER — stable model

This file belongs to the owner and is created only when absent. Record stable
machine constraints, communication preferences, delegated decisions, and
actions that always require confirmation. Record a settled SLP decision with
\`maestro decide "<choice>" --why "<reason>"\`; use \`--replaces\` when a later
decision supersedes it.
`;

const shellrc = `function _maestro_home() {
  local workspace_id root_pane_id
  workspace_id="$(herdr workspace list | bun -e 'const input = JSON.parse(await Bun.stdin.text()); const workspace = input.result.workspaces.find((candidate) => candidate.label === "maestro"); if (workspace) process.stdout.write(workspace.workspace_id);')"
  if [[ -z "$workspace_id" ]]; then
    workspace_id="$(herdr workspace create --cwd "$HOME/maestro" --label maestro --no-focus | bun -e 'const input = JSON.parse(await Bun.stdin.text()); process.stdout.write(input.result.workspace.workspace_id);')"
  fi
  root_pane_id="$(herdr agent list | HERDR_WORKSPACE_ID="$workspace_id" bun -e 'const input = JSON.parse(await Bun.stdin.text()); const workspace = Bun.env.HERDR_WORKSPACE_ID; const agent = input.result.agents.find((candidate) => candidate.name === "supervisor" && candidate.workspace_id === workspace); if (agent) process.stdout.write(agent.pane_id);')"
  if [[ -z "$root_pane_id" ]]; then
    root_pane_id="$(herdr tab create --workspace "$workspace_id" --cwd "$HOME/maestro" --label supervisor --no-focus | bun -e 'const input = JSON.parse(await Bun.stdin.text()); process.stdout.write(input.result.root_pane.pane_id);')"
    herdr agent start supervisor --kind claude --pane "$root_pane_id" >/dev/null
  fi
  herdr agent focus supervisor >/dev/null
}

alias hm=_maestro_home
`;

const retiredRoomFiles = [
  "lane.md",
  "lead.md",
  "observer.md",
  "supervisor.md",
  "observer-watch.sh",
] as const;

export async function scaffoldRoom(home: string): Promise<string> {
  const room = join(home, "maestro");
  await mkdir(room, { recursive: true });
  await chmod(room, 0o700);
  for (const [name, content] of [
    ["IDENTITY.md", identity],
    ["AGENTS.md", agents],
    ["CLAUDE.md", agents],
    ["shellrc", shellrc],
  ] as const) {
    await writeFile(join(room, name), content);
  }
  try {
    await writeFile(
      join(room, "SLP.md"),
      await readFile(join(import.meta.dir, "resources", "SLP.md")),
      { flag: "wx" },
    );
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "EEXIST") throw error;
  }
  for (const retired of retiredRoomFiles) {
    await rm(join(room, retired), { force: true });
  }
  try {
    await writeFile(join(room, "OWNER.md"), owner, { flag: "wx" });
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "EEXIST") throw error;
  }
  await chmod(join(room, "OWNER.md"), 0o600);
  return room;
}
