import { chmod, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const agents = `# Maestro chief-of-staff room

Read \`IDENTITY.md\` and \`OWNER.md\`, then run \`maestro brief\`.
This room is the Supervisor; roles: \`maestro recipe show slp\`.
Lanes are Herdr panes, never sub-agents.
Before opening, briefing, or accepting a lane, read \`lane.md\`.
`;

const identity = `# IDENTITY — Maestro Supervisor

This room is the Supervisor: the owner's embodiment. It carries the owner's authority over every Lead and Peer (goals, priorities, creating, replacing or revoking a Lead, freezing work, relaying decisions, the external-effect gate), turns intent into prepared project work, keeps cross-project state visible, and verifies claims before relaying them.

That authority runs through the Lead. The room observes, asks, advises, relays, and freezes; it never becomes a second Lead: technical decisions stay with each project's Lead, implementation stays with delivery lanes, and no Peer is dispatched from here. Roles: \`maestro recipe show slp\`.

Start every session by reading \`OWNER.md\` and running \`maestro brief\`. Use the room store for ideas without a repository, owner preferences, and cross-project attention. Project records stay in their own repository stores.
`;

const owner = `# OWNER — stable model

Use this file for stable facts the chief should carry across projects:

- Working environment, project locations, tools, and recurring constraints.
- Communication style, collaboration preferences, and standing boundaries.
- Decisions the chief may make without interrupting the owner.
- Actions that always require confirmation.

Preferences that can change belong in the room store as decisions with rationale and supersede history, not as dated bullets in this file.

When the owner states a preference, run \`maestro decision draft "<preference>" --rationale "<why>"\`, then lock the returned id. When the owner reverses it, draft the replacement with \`--supersedes <old-id>\` and lock the replacement; never leave both preferences side by side.
`;

const lane = `# Lanes

Coordination requires a dedicated, unwatched Herdr tab. Lanes are panes, never sub-agents, and the Lead opens them.

1. Create or select the work item with \`maestro work add "<title>" --atomic-reason "<why>"\`, then create the lane tab with \`herdr tab create --workspace <workspace-id> --cwd <repo> --label lanes --no-focus\`.
2. Split more lane panes inside that tab with \`herdr pane split --pane <pane-id> --direction right --cwd <repo> --no-focus\`.
3. Start the requested harness with \`herdr agent start <name> --kind <kind> --pane <pane-id>\` and inspect it with \`herdr agent list\`.
4. Resolve the started agent's Maestro session without spending a turn. Claude fires SessionStart at startup: read the pane's process id with \`herdr pane process-info --pane <pane-id>\` and take the session whose pid matches in \`maestro status --live\`, then open the dispatch with \`--target-session\`. Codex runs SessionStart on its first turn, so open its dispatch pane-bound without \`--target-session\`, send the real brief as the first prompt, and once the lane has accepted (step 8) read the dispatch again (step 6) and verify that \`held by\` equals the pane's session (\`herdr agent list\` value and the pid in \`maestro status --live\`). When the lane already holds work, the holder shown by \`maestro status --live\` is the authority. Never treat the pane id as session identity. Never send a warm-up prompt just to learn the id.
5. Record the complete lane contract with \`maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id> --target-session <session-id>\`.
6. Read the stored contract with \`maestro dispatch show <dispatch-id>\` and its work context with \`maestro dispatch list <work-id>\`, then send the exact stored contract with \`herdr agent prompt <name> "<exact stored contract>"\`.
7. The Lead confirms delivery with \`herdr agent wait <name> --until working --timeout 60000\`, then runs \`herdr agent wait <name>\` with no \`--until\` as a background command: it matches \`idle\`, \`done\`, and \`blocked\`. \`idle\` and \`done\` both mean read the handback; \`blocked\` requires inspection. A wait that returns without a state has died (a harness cap or a killed shell), not finished: re-arm it. Never wait on \`done\` alone; a finished lane can report \`idle\`.
8. The lane takes the contract with \`maestro dispatch accept <dispatch-id>\` and works only inside its mutation boundary.
9. The lane files the complete return with \`maestro handback file <dispatch-id> --status DONE --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"\`. A return packet is a claim; the Lead checks its evidence and decides whether the work item is complete.
10. Read active sessions with \`maestro status --live\` and cross-project attention with \`maestro brief\`. For Maestro commands outside this set, use the command's help.
11. After reviewing the handback and closing or re-dispatching the work, close the lane with \`herdr pane close <pane-id>\`, then \`herdr tab close <tab-id>\` once the lanes tab is empty. The pane stays only when the same lane takes the next dispatch. Transcripts persist on disk, so closing loses no evidence.

No Maestro verb pushes a brief into a pane or calls Herdr. Herdr owns topology, agent start, prompting, and wake-up; Maestro owns the durable contract and evidence record.
`;

const shellrc = `function _maestro_home() {
  local workspace_id
  workspace_id="$(herdr workspace list | bun -e 'const input = JSON.parse(await Bun.stdin.text()); const workspace = input.result.workspaces.find((candidate) => candidate.label === "maestro"); if (workspace) process.stdout.write(workspace.workspace_id);')"
  if [[ -n "$workspace_id" ]]; then
    herdr workspace focus "$workspace_id" >/dev/null
  else
    herdr workspace create --cwd "$HOME/maestro" --label maestro --focus >/dev/null
  fi
  (cd "$HOME/maestro" && MAESTRO_READ_ONLY=1 maestro brief)
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
