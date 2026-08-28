import type { Database } from "bun:sqlite";
import { chmod, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

export const installInRoomMessage =
  "~/maestro is the Supervisor room, not a repository; run maestro install from a repository checkout, which maintains the room";

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

const agents = `# Maestro chief-of-staff room

Read \`IDENTITY.md\` and \`OWNER.md\`. While \`OWNER.md\` still holds unanswered questions, interview the owner and write the answers there before anything else; then run \`maestro brief\`.
This room is the Supervisor; roles: \`maestro recipe show slp\`. Repository-only verbs are \`maestro install\`, \`maestro update\`, and \`maestro uninstall\`; \`maestro doctor\` wiring checks describe repositories, not this room.
Lanes are Herdr panes, never sub-agents.
Before opening, briefing, or accepting a lane, read \`lane.md\`.
Before handing owner intent to a repository, read \`lead.md\`.
The room never edits any store by hand (no sqlite, no file edits under \`.maestro\`); every store changes only through \`maestro\` verbs, and a defect in stored data is owner intent for the Lead of the code that wrote it, relayed per \`lead.md\`.
`;

const identity = `# IDENTITY — Maestro Supervisor

This room is the Supervisor: the owner's embodiment. It carries the owner's authority over every Lead and Peer (goals, priorities, creating, replacing or revoking a Lead, freezing work, relaying decisions, the external-effect gate), turns intent into prepared project work, keeps cross-project state visible, and verifies claims before relaying them.

That authority runs through the Lead. The room observes, asks, advises, relays, and freezes; it never becomes a second Lead: technical decisions stay with each project's Lead, implementation stays with delivery lanes, and no Peer is dispatched from here. Roles: \`maestro recipe show slp\`.

Start every session by reading \`OWNER.md\` and running \`maestro brief\`. Use the room store for ideas without a repository, owner preferences, and cross-project attention. Project records stay in their own repository stores.

Before handing owner intent to a repository, read \`lead.md\`.

## Binding

- Owner: the person named in \`OWNER.md\`
- Project scope: every repository in \`registry\`
- Reporting target: the owner, through \`maestro brief\` packets and decisions in this store
- Observation boundary: stores, handbacks, and attention of registered repositories
- Raw transcript access: denied
- Write authority: none
- Acceptance authority: none
- Recovery or replacement lease: none
- Review date: set by the owner in \`OWNER.md\`

The room store is the notebook: record only novel or material observations, aggregated by pattern. When stuck, escalate with a packet whose \`human decision needed\` is \`yes\` and wait; never open a second Supervisor. Any expansion of this binding is written here by the owner.
`;

const owner = `# OWNER — stable model

This file belongs to the owner: the installer writes it once and it is never overwritten. Until the questions below are answered it is a template, and the Supervisor's first session is an interview: ask them, write the answers here, and only then run \`maestro brief\`.

Use this file for stable facts the chief should carry across projects:

- Working environment, project locations, tools, and recurring constraints.
  - Where does code live, and which terminal workspace manager opens panes?
  - Which harnesses run here, and which one for which kind of work?
  - Which examples should the owner-editable Model table column use for cheap, strong, diverse, and lead? Keep the current names here.
  - What is always true about this machine: toolchains, shared checkouts, commands that must never run?
- Communication style, collaboration preferences, and standing boundaries.
  - Which language, how terse, and what should never appear in a reply?
  - How should questions be put: one at a time, with options, in prose?
- Decisions the chief may make without interrupting the owner.
  - Which of these are free: implementation details, sequencing, test strategy, retries, opening lanes?
- Actions that always require confirmation.
  - Which of these are gated: push, tag, publish, deploy, delete, spend, credentials, scope changes?

Preferences that can change belong in the room store as decisions with rationale and supersede history, not as dated bullets in this file.

When the owner states a preference, run \`maestro decision draft "<preference>" --rationale "<why>"\`, then lock the returned id. When the owner reverses it, draft the replacement with \`--supersedes <old-id>\` and lock the replacement; never leave both preferences side by side.
`;

const lane = `# Lanes

Coordination requires a dedicated, unwatched Herdr tab. Lanes are panes, never sub-agents, and the Lead opens them.

A lane never uses \`herdr pane send-text\`. It reaches its Lead with \`herdr agent prompt lead-<repo basename> "[from peer][x<id>] <message>"\` about a stored record (a handback, a note, a draft). Its returns stay the handback and \`--request\`. A lane never messages the Supervisor.

A session whose pane name starts with \`peer-\` treats any prompt that is not its stored dispatch contract as not its role. It replies exactly \`not my role: <name> holds <dispatch id>; send intent to the Lead\`, runs no Maestro write verb and files nothing.

\`<owner>\` is the repository basename when a Lead opens the tab (for example, \`lanes-maestro-w524\`) and the literal \`room\` when the Supervisor opens lanes for room-store work (for example, \`lanes-room-w522\`), because the room directory is also named \`maestro\` and its basename cannot distinguish room-store work.

1. Create or select the work item with \`maestro work add "<title>" --atomic-reason "<why>"\`, then create the lane tab with \`herdr tab create --workspace <workspace-id> --cwd <repo> --label lanes-<owner>-<work id> --no-focus\`.
2. Split more lane panes inside that tab with \`herdr pane split --pane <pane-id> --direction right --cwd <repo> --no-focus\`.
3. Choose one of five lane types: \`scout\`, \`decision\`, \`delivery\`, \`challenge\`, or \`shadow\`; the dispatch id will be the agent's role name. Before opening a delivery dispatch, the Lead releases its own work lease with \`maestro work release <work-id>\`; otherwise the lane never runs maestro work start.
4. With the tab and pane already present, open the complete contract before starting the agent: \`maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id>\`. Open it pane-bound without \`--target-session\`.
5. Start the assigned role with \`herdr agent start peer-<dispatch id> --kind <kind> --pane <pane-id>\`. Pass the chosen model from the Model table in \`maestro recipe show slp\` and the lane's thinking level from its table to the harness: use \`-- --model <name> --effort <level>\` for Claude or Codex's \`--model <name> -c model_reasoning_effort=<level>\` flags (verified with \`claude --help\` and \`codex --help\`). Inspect the agent with \`herdr agent list\`; this applies to \`scout\`, \`decision\`, \`delivery\`, \`challenge\`, and \`shadow\`. Read the stored contract with \`maestro dispatch show <dispatch-id>\` and its work context with \`maestro dispatch list <work-id>\`, then send the exact stored contract with \`herdr agent prompt peer-<dispatch id> "<exact stored contract>"\`. One lane per \`herdr agent prompt\` call, always naming \`peer-<dispatch id>\`, never a shell loop over lanes: a mis-passed argument returns success without delivering. Every later message between roles starts with \`[from <role>]\` and the record id it is about (\`maestro recipe show slp\`, Talking across roles).
6. The lane takes the contract with \`maestro dispatch accept <dispatch-id>\` and works only inside its mutation boundary. A delivery lane works under that accepted dispatch. A lane that hits \`LEASE_HELD\` returns \`BLOCKED\` and names the holder from the error.
7. Resolve the accepted session without spending a turn. Claude fires SessionStart at startup and Codex runs SessionStart on its first turn. Read the pane's process id with \`herdr pane process-info --pane <pane-id>\`, take the session whose pid matches in \`maestro status --live\`, and verify that \`claimed by\` or \`held by\` equals the pane's session; the \`herdr agent list\` value must match it. When \`claimed by\` matches, the Lead runs \`maestro dispatch confirm <dispatch-id> --session <session-id>\`. On a mismatch, the Lead runs \`maestro dispatch cancel <dispatch-id> --reason wrong-holder\` and opens a new dispatch. When the lane already holds work, the holder shown by \`maestro status --live\` is the authority. Never treat the pane id as session identity. Never send a warm-up prompt just to learn the id. The Lead must confirm \`working\` before briefing the next lane with \`herdr agent wait peer-<dispatch id> --until working --timeout 60000\`, then run \`herdr agent wait peer-<dispatch id>\` with no \`--until\` as a background command. It matches \`idle\`, \`done\`, and \`blocked\`; \`idle\` and \`done\` mean read the handback, while \`blocked\` requires inspection. A wait with no state has died, not finished: re-arm it. Never wait on \`done\` alone. The wait is a convenience; the handback in the store is the return; a wait that outlives the handback (a lane with a background shell stays \`working\`) is resolved by reading the store, never by prompting the lane.
8. A delivery lane passes \`--candidate <commit or digest>\` with its DONE handback. It files the complete return with \`maestro handback file <dispatch-id> --status DONE --candidate "<commit or digest>" --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"\`; other lanes may omit the optional candidate. \`BLOCKED\`, \`DEPENDENCY_REQUEST\`, \`COUNCIL_REQUEST\`, and \`REOPEN_REQUEST\` also pass \`--request "<retry condition or requested action>"\`. A return packet is a claim; the Lead checks its evidence and decides whether the work item is complete. The lane must file exactly once, when the stop condition is met; a lane with a second stop point needs a second dispatch. Evidence that arrives later is \`maestro work note <work-id> "after h<id>: <evidence>"\` plus a \`[from peer]\` message; it never changes the filed handback.
9. Read cross-project attention with \`maestro brief\`. For Maestro commands outside this set, use the command's help.
10. After reviewing the handback and closing or re-dispatching the work, close the lane with \`herdr pane close <pane-id>\`, then \`herdr tab close <tab-id>\` once the \`lanes-<owner>-<work id>\` tab is empty. The pane stays only when the same lane takes the next dispatch. A \`[from lead]\` prompt never changes an assignment: a new objective or a new stop point is a new dispatch (step 4) on the same work item and the same pane, naming the predecessor handback in the objective. Transcripts persist on disk, so closing loses no evidence.
11. A finding returned in a handback is closed by that handback; it becomes a card only when it is the next thing the Lead will actually do.
12. Cross-examination, only when first views conflict or the risk warrants it: open a second generation of dispatches on the same work item, paste the other lanes' handbacks verbatim into each contract with one targeted question, and read the answers as handbacks (DONE with a CONFIRM claim, CHALLENGE, or REOPEN_REQUEST; CONFIRM is claim text, not a status). Lanes never prompt each other; the Lead reconciles.

No Maestro verb pushes a brief into a pane or calls Herdr. Herdr owns topology, agent start, prompting, and wake-up; Maestro owns the durable contract and evidence record.
`;

const lead = `# Handing owner intent to a repository Lead

The room relays owner intent to the repository Lead without taking project authority.

1. When the owner states intent in the room, find the repository in \`registry\`. When the owner says fix or do, relay without asking whether to relay; ask the owner a question only for a real fork.
2. Run \`herdr agent list\`. The room finds a Lead only as a Herdr agent named \`lead-<repo basename>\` whose cwd is the repository; every other pane is absent. Never prompt a pane with any other name.
3. If that exact agent exists, run \`herdr agent prompt lead-<repo basename> "[from supervisor][intent] <owner words verbatim>"\`.
4. If it does not exist, run \`herdr tab create --workspace <workspace-id> --cwd <repo> --label lead-<repo basename> --no-focus\`, then \`herdr agent start lead-<repo basename> --kind <harness OWNER.md names> --pane <pane-id>\`. Pick the Lead's model from the \`lead\` rung of the Model table in \`maestro recipe show slp\`. Run \`herdr agent prompt lead-<repo basename> "[from supervisor][intent] <owner words verbatim>. You are the Lead of <repo>; this is owner intent relayed by the room; record it as work and choose your own route (d700)."\`, then \`herdr agent wait lead-<repo basename> --until working --timeout 60000\`.
5. In the room store, run \`maestro work note <room-work-id> "handed intent to <repo>: <one-line summary>"\`.
6. Never run \`maestro work add\` or any write in the project store, run \`maestro dispatch open\`, suggest topology in the prompt, or read the pane transcript. When the Lead needs a room decision, run \`maestro decision draft "<the choice>" --rationale "<why, options>" --work <id>\`, then \`herdr agent prompt supervisor "[from lead][ask d<id>] <question>"\`. A non-decision question is a work note sent the same way. The room never runs \`herdr agent wait\` on a Lead: Herdr reports \`working\` while any background shell lives, the store is the truth and the room's next prompt shows it. The room's reply is a prompt and the record (lock or supersede) is what the Lead acts on.
`;

const shellrc = `function _maestro_home() {
  local workspace_created=0 workspace_id workspace_pane_ids root_pane_id supervisor_pane_id
  workspace_id="$(herdr workspace list | bun -e 'const input = JSON.parse(await Bun.stdin.text()); const workspace = input.result.workspaces.find((candidate) => candidate.label === "maestro"); if (workspace) process.stdout.write(workspace.workspace_id);')"
  if [[ -n "$workspace_id" ]]; then
    herdr workspace focus "$workspace_id" >/dev/null
  else
    herdr workspace create --cwd "$HOME/maestro" --label maestro --focus >/dev/null
    workspace_created=1
    workspace_id="$(herdr workspace list | bun -e 'const input = JSON.parse(await Bun.stdin.text()); const workspace = input.result.workspaces.find((candidate) => candidate.label === "maestro"); if (workspace) process.stdout.write(workspace.workspace_id);')"
  fi
  workspace_pane_ids="$(herdr pane list --workspace "$workspace_id" | bun -e 'const input = JSON.parse(await Bun.stdin.text()); process.stdout.write(input.result.panes.map((candidate) => candidate.pane_id).join(","));')"
  supervisor_pane_id="$(herdr agent list | HERDR_WORKSPACE_PANE_IDS="$workspace_pane_ids" bun -e 'const input = JSON.parse(await Bun.stdin.text()); const paneIds = new Set((Bun.env.HERDR_WORKSPACE_PANE_IDS ?? "").split(",").filter(Boolean)); const agent = input.result.agents.find((candidate) => candidate.name === "supervisor" && paneIds.has(candidate.pane_id)); if (agent) process.stdout.write(agent.pane_id);')"
  (cd "$HOME/maestro" && MAESTRO_READ_ONLY=1 maestro brief)
  if [[ -n "$supervisor_pane_id" ]]; then
    herdr agent focus supervisor >/dev/null
  else
    if [[ "$workspace_created" -eq 1 ]]; then
      root_pane_id="\${workspace_pane_ids%%,*}"
    else
      root_pane_id="$(herdr tab create --workspace "$workspace_id" --cwd "$HOME/maestro" --label supervisor | bun -e 'const input = JSON.parse(await Bun.stdin.text()); process.stdout.write(input.result.root_pane.pane_id);')"
    fi
    # The owner may edit the supervisor's agent kind.
    herdr agent start supervisor --kind claude --pane "$root_pane_id" >/dev/null
  fi
}

alias hm=_maestro_home

function maestro_lanes() {
  if [[ $# -ne 1 ]]; then
    printf '%s\\n' 'usage: maestro_lanes <work-id>' >&2
    return 2
  fi
  local lanes
  lanes="$(maestro dispatch list "$1")" || return
  printf '%s\\n' "$lanes" | bun -e 'const input = await Bun.stdin.text(); let agents = []; try { const result = Bun.spawnSync(["herdr", "agent", "list"], { stdout: "pipe", stderr: "ignore" }); if (result.exitCode === 0) agents = JSON.parse(new TextDecoder().decode(result.stdout)).result?.agents ?? []; } catch {} const statuses = new Map(agents.map((agent) => [agent.pane_id, agent.agent_status])); process.stdout.write(input.split("\\n").map((line) => { const pane = line.startsWith("lane ") ? line.slice(5).split(" | ")[0] : null; return pane ? line + " | agent=" + (statuses.get(pane) ?? "unknown") : line; }).join("\\n"));'
}
`;

export async function scaffoldRoom(home: string): Promise<string> {
  const room = join(home, "maestro");
  await mkdir(room, { recursive: true });
  await chmod(room, 0o700);
  for (const [name, content] of [
    ["IDENTITY.md", identity],
    ["AGENTS.md", agents],
    ["CLAUDE.md", agents],
    ["lane.md", lane],
    ["lead.md", lead],
    ["shellrc", shellrc],
  ] as const) {
    await writeFile(join(room, name), content);
  }
  try {
    await writeFile(join(room, "OWNER.md"), owner, { flag: "wx" });
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "EEXIST") throw error;
  }
  await chmod(join(room, "OWNER.md"), 0o600);
  return room;
}
