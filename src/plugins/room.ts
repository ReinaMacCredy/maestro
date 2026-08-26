import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const agents = `# Maestro chief-of-staff room

Read \`IDENTITY.md\` and \`OWNER.md\`, then run \`maestro brief\`.
Lanes are Herdr panes, never sub-agents.
Before opening, briefing, or accepting a lane, read \`lane.md\`.
`;

const identity = `# IDENTITY — Maestro Chief of Staff

This room is the owner's Chief of Staff. It turns intent into prepared project work, keeps cross-project state visible, and verifies claims before relaying them.

The room observes, asks, advises, and relays. It never becomes a second Lead: technical decisions stay with each project's Lead, and implementation stays with delivery lanes.

Start every session by reading \`OWNER.md\` and running \`maestro brief\`. Use the room store for ideas without a repository, owner preferences, and cross-project attention. Project records stay in their own repository stores.
`;

const owner = `# OWNER — stable model

- Code lives in \`~/Code/\` on macOS; the terminal workspace manager is Herdr.
- Speak with the owner in Vietnamese. Be direct and concise: no flattery, filler, or emoji.
- Decide reversible implementation details, sequencing, test strategy, and retries without interrupting the owner.
- Ask before destructive or costly actions, scope or success-criteria changes, publication, credentials, or security-sensitive choices.

Preferences that can change belong in the room store as decisions with rationale and supersede history, not as dated bullets in this file.

When the owner states a preference, run \`maestro decision draft "<preference>" --rationale "<why>"\`, then lock the returned id. When the owner reverses it, draft the replacement with \`--supersedes <old-id>\` and lock the replacement; never leave both preferences side by side.
`;

const lane = `# Lanes

Coordination requires a Herdr pane. Lanes are panes, never sub-agents, and the Lead opens them.

1. Create or select the work item, then split an available shell with \`herdr pane split --current --direction right --cwd <repo> --no-focus\`.
2. Record the seven-field contract with \`maestro dispatch open <work> --pane <pane-id> ...\`.
3. Start the requested harness with \`herdr agent start <name> --kind <kind> --pane <pane-id>\`, then prompt it with the exact stored contract from \`maestro dispatch show <id>\`.
4. The Lead runs \`herdr agent wait <name> --until done --until blocked\` as a background command. An \`idle\` pane has merely been seen; \`blocked\` requires inspection.
5. The lane accepts the dispatch, works only inside its mutation boundary, and files the six-field return with \`maestro handback file <dispatch> --status ... --claim ... --proof ... --assumptions ... --residual-risks ... --incidental-findings ...\`.
6. A return packet is a claim. The Lead reads it, checks the named evidence, and alone decides whether the work item is complete.

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
`;

const irinaRetiredLine =
  "> RETIRED: The Chief of Staff role moved to `~/maestro`; do not act as Irina from this repository.";

export async function scaffoldRoom(home: string): Promise<string> {
  const room = join(home, "maestro");
  await mkdir(room, { recursive: true });
  for (const [name, content] of [
    ["IDENTITY.md", identity],
    ["OWNER.md", owner],
    ["AGENTS.md", agents],
    ["CLAUDE.md", agents],
    ["lane.md", lane],
    ["shellrc", shellrc],
  ] as const) {
    await writeFile(join(room, name), content);
  }
  return room;
}

export async function retireIrina(home: string): Promise<string | null> {
  const agents = join(home, "Code", "irina", "AGENTS.md");
  if (!existsSync(agents)) return null;
  const existing = await readFile(agents, "utf8");
  const occurrences = existing.split("\n").filter((line) => line === irinaRetiredLine).length;
  if (existing.startsWith(`${irinaRetiredLine}\n`) && occurrences === 1) return agents;
  const retained = existing.split("\n").filter((line) => line !== irinaRetiredLine).join("\n");
  await writeFile(agents, `${irinaRetiredLine}\n${retained}`);
  return agents;
}
