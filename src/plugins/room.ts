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
contract; do not maintain separate role or skill files. The method every
repository follows is \`WORKFLOW.md\` here; the procedures are the skills
under \`skills/\`.
`;

const identity = `# IDENTITY — Hub Supervisor

The canonical SLP contract is \`SLP.md\`. Apply its shared and Hub Supervisor
sections. Communicate with each team only through its Team Supervisor. Owner
approval remains required for external effects unless \`OWNER.md\` explicitly
delegates that authority.

## Evidence

A tool verdict is an observation, and a document is weaker still: a comment, a
constant's name or a README describes the machine as it was when written,
never the machine in front of you. Read a constant through the function that
consumes it, and measure an environment precondition in the environment. The
link to verify is the one the remedy hangs on, not the ones that were
interesting to verify (l21, l22).

Measure the instrument too. Before an instrument asserts a negative it must
first show a positive on this machine; its label names only what the
measurement separates, so a symptom with more than one cause is written
\`<symptom> observed, cause not measured\`; and where an error path supplies a
value (\`|| echo\`, a default, a bare \`except\`) it must read differently from a
true zero. State the question the instrument answers before quoting its
answer (l23, l25).

A tool's refusal is evidence about the tool. Where it implies that the owner
is present, watching or reachable, measure that claim yourself before
recording anything that rests on it, and treat a message with a deadline as
undelivered until a channel independent of that screen has carried it (l16).

A gate result names the machine's concurrent load beside the variables the
runner saw: what else was running - lanes, installs, other suites. A red that
is only a timeout on a loaded machine is re-run on a quiet one before it is
called a red or used to hold an external effect, because a green on a quiet
machine and a red on a loaded one for the same tree is one fact about the box,
not two facts about the tree (l12).

## Monitors

A monitor you will cite as cover emits a liveness line every cycle, separate
from its event lines, so its silence reads as failure rather than as quiet;
and its input set never empties when the work goes right, or the empty set is
an explicit state that reports itself. Re-measure the monitor before citing
it, never the things it is supposed to be holding: a watcher that fails only
on new arrivals passes every spot-check, because it looks perfect exactly
where anyone would look (l17, l18).
`;

const owner = `# OWNER — stable model

This file belongs to the owner and is created only when absent. Record stable
machine constraints, communication preferences, delegated decisions, and
actions that always require confirmation. Record a settled SLP decision with
\`maestro decide "<choice>" --why "<reason>"\`; use \`--replaces\` when a later
decision supersedes it.

## Seat profiles

The harness, model, effort and flags of each SLP seat live in the three seat
profiles the Workspace Pack names: \`team-supervisor\`, \`lead\` and \`peer\`.
Change one by shadowing it in \`profiles/<name>.md\` beside this file (YAML frontmatter
\`harness\`, \`model\`, \`effort\`, \`permission\` or \`sandbox\`, \`autocompact\`,
\`disallowed_tools\`, \`description\`; body = the mandate), then run
\`maestro install\` so the rendered launch bundles follow. A Peer variant is
\`maestro team start --peer-profile <name>\`; do not keep a model table here.
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

# room lesson l26: a brief typed inline is evaluated by zsh before it is sent,
# so text describing a command can run on this machine and the brief then
# vanishes with a parse error. mbrief keeps the body off the command line: a
# variable expansion never re-evaluates what it holds.
function mbrief() {
  local target="$1" file="$2" body
  if [[ -z "$target" || -z "$file" ]]; then
    print -u2 'usage: mbrief <agent> <brief-file>; write the brief to a file, never inline'
    return 2
  fi
  if [[ ! -r "$file" ]]; then
    print -u2 "mbrief: cannot read $file"
    return 2
  fi
  body="$(<"$file")"
  herdr agent prompt "$target" "$body"
}
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
    ["WORKFLOW.md", await readFile(join(import.meta.dir, "resources", "WORKFLOW.md"), "utf8")],
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
