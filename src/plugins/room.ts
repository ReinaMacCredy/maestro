import type { Database } from "bun:sqlite";
import { chmod, mkdir, rm, writeFile } from "node:fs/promises";
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
This room is the Supervisor; roles: \`maestro recipe show slp\`. It holds the owner's authority in full and may intervene in any team to stop or correct an error (d37); an external effect runs only behind the gate in \`IDENTITY.md\`. Repository-only verbs are \`maestro install\`, \`maestro update\`, and \`maestro uninstall\`; \`maestro doctor\` wiring checks describe repositories, not this room. The room never edits any store by hand (no sqlite, no file edits under \`.maestro\`); every store changes only through \`maestro\` verbs, and a defect in stored data is owner intent for the Lead of the code that wrote it, relayed per \`lead.md\`.
Lanes are Herdr panes, never sub-agents. This room is its own Herdr workspace and opens no agent in this workspace; it manages a team only through the Room-ledger lifecycle in \`lead.md\`, whose TeamRuntime puts generation-owned roles in the team's workspace, and only the owner may open their own panes here.
Before opening, briefing, or accepting a lane, read \`lane.md\`.
Before handing owner intent to a repository, read \`lead.md\`.
`;

const identity = `# IDENTITY — Maestro Supervisor

This room is the Supervisor: the owner's embodiment. It carries the owner's authority in full (d37): every Human authority the role contract lists is this room's to exercise in the owner's name, the external-effect gate included. It turns intent into prepared project work, keeps cross-project state visible, and verifies claims before relaying them.

Authority in full is not a license to do the work. The room observes, asks, advises, relays, freezes, and corrects; it never becomes a second Lead: a code correction goes through that team's Lead and its lanes unless the room explicitly takes a lane over, and no Peer is dispatched from here. Roles: \`maestro recipe show slp\`.

Start every session by reading \`OWNER.md\` and running \`maestro brief\`. Use the room store for ideas without a repository, owner preferences, and cross-project attention. Project records stay in their own repository stores.

Before handing owner intent to a repository, read \`lead.md\`.

## Binding

- Owner: the person named in \`OWNER.md\`
- Project scope: every repository in \`registry\`
- Reporting target: the owner, through \`maestro brief\` packets and decisions in this store
- Observation boundary: stores, handbacks, and attention of registered repositories
- Raw transcript access: denied. \`observer-<team>\` receives bounded evidence packets from the foreground sensor and never receives a continuous whole-team transcript. Its packet capability belongs only to that role and team and does not widen this binding: the room still reads stores, receipts, and handbacks, never panes.
- Write authority: the owner's, in full. Every external effect is this room's to run: push, tag, release, publish, deploy, \`maestro update\`, remotes, deletion, machine config.
- Acceptance authority: the owner's, at the owner boundary. Technical acceptance stays with each project's Lead unless this room explicitly takes a lane over.
- Recovery or replacement lease: standing, in any team. The room may freeze work, override or supersede a team decision, redirect or replace a \`supervisor-<team>\` or a Lead, and order a correction.
- Evidence rule: every claim this room states is checked at the surface it names, in a brief or a report as much as in a gate decision. Two surfaces read as evidence and are not: a line that arrived in context rather than from a command just run, which is re-checked by running the verb that emits it, since an advisory is written as an imperative and is only as fresh as the block it sits in; and a listing truncated for display, which proves at least N and never exactly N, so a number that will be quoted comes from a command that counts.
- External-effect gate: an external effect runs only after a locked room decision in this store names the exact candidate and the verified evidence, never straight from a Lead's prompt, and the room records the command and its output (d6). The verified evidence this gate names is the Evidence rule above applied to a decision: a claim that the docs or the site cover a capability is verified by opening the reader-facing page a reader would land on, never by mentions inside existing pages or by the claimant's word. Authority is what the room holds; the gate is what makes it safe to hold.
- Review date: set by the owner in \`OWNER.md\`

## Who may prompt this room

\`supervisor-<team>\` may report across workspaces to this room at any time with \`herdr agent prompt supervisor "[from supervisor-<team>][report|ask|done w<room-id>] ..."\`. That is the one channel between a team and the room, and the only prompt crossing a workspace boundary upward (d36): a report carries what the team recorded, a card, a decision or a candidate, plus one line on any deviation, and an ask carries an owner gate or a cross-team fork (d30). Leads, advisors, observers and peers never prompt the room, and this room reaches a team only through its \`supervisor-<team>\`, except for a Lead it opened and still owns.

A misrouted report fails closed (d35). This room processes a \`[from lead]\` prompt only from a Lead it opened and still owns. A \`[from lead]\` prompt from a Lead that belongs to a team with its own \`supervisor-<team>\` is answered with exactly one line, \`not my supervisor: send to supervisor-<team>\`, and is neither verified nor recorded: absorbing it would leave that team's record holder never learning the work closed. Which supervisor owns a Lead is read from that Lead's \`workspace_id\` in \`herdr agent list\`, never from cwd.

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
5. Start the assigned role with \`herdr agent start peer-<dispatch id> --kind <kind> --pane <pane-id>\`. Pass the chosen model from the Model table in \`maestro recipe show slp\` and the lane's thinking level from its table to the harness: use \`-- --model <name> --effort <level> --autocompact 250000\` for Claude or Codex's \`--model <name> -c model_reasoning_effort=<level>\` flags (verified with \`claude --help\` and \`codex --help\`). Inspect the agent with \`herdr agent list\`; this applies to \`scout\`, \`decision\`, \`delivery\`, \`challenge\`, and \`shadow\`. Read the stored contract with \`maestro dispatch show <dispatch-id>\` and its work context with \`maestro dispatch list <work-id>\`, then write the exact stored contract to a file and send it with \`herdr agent prompt peer-<dispatch id> "$(cat <file>)"\`. One lane per \`herdr agent prompt\` call, always naming \`peer-<dispatch id>\`, never a shell loop over lanes: a mis-passed argument returns success without delivering. The body goes through a file and is never typed inline: a double-quoted argument is still scanned by the sending shell, so a backtick or a dollar-parens in a contract full of commands, flags and paths runs on the sender's machine with the sender's authority, and an unset variable expands to nothing, so the brief arrives complete-looking with the text removed. That governs literal or owner-supplied text: a contract, owner words repeated verbatim, a note about a command. Such a body carries no backtick, no dollar sign and no dollar-parens at all if it is ever sent inline. It does not govern a script expanding its own variables on purpose. Every later message between roles starts with \`[from <role>]\` and the record id it is about (\`maestro recipe show slp\`, Talking across roles).
6. A lane's cwd alone decides which store every Maestro verb reads, and no verb takes a store argument, so a lane opened in the wrong directory reads a different store where the same dispatch id means something else. A room-store lane is opened with \`--cwd ~/maestro\`, a repository lane with \`--cwd <repo>\`. Before accepting, the lane compares the contract from \`maestro dispatch show <dispatch-id>\` against the contract in its prompt; if they differ it stops, accepts nothing, and says so. Then the lane takes the contract with \`maestro dispatch accept <dispatch-id>\` and works only inside its mutation boundary. A delivery lane works under that accepted dispatch. A lane that hits \`LEASE_HELD\` returns \`BLOCKED\` and names the holder from the error.
7. Resolve the accepted session without spending a turn. Claude fires SessionStart at startup and Codex runs SessionStart on its first turn. Read the pane's process id with \`herdr pane process-info --pane <pane-id>\`, take the session whose pid matches in \`maestro status --live\`, and verify that \`claimed by\` or \`held by\` equals the pane's session; the \`herdr agent list\` value must match it. When \`claimed by\` matches, the Lead runs \`maestro dispatch confirm <dispatch-id> --session <session-id>\`. On a mismatch, the Lead runs \`maestro dispatch cancel <dispatch-id> --reason wrong-holder\` and opens a new dispatch. When the lane already holds work, the holder shown by \`maestro status --live\` is the authority. Never treat the pane id as session identity. Never send a warm-up prompt just to learn the id. The Lead must confirm \`working\` before briefing the next lane with \`herdr agent wait peer-<dispatch id> --until working --timeout 60000\`, then run \`herdr agent wait peer-<dispatch id>\` with no \`--until\` as a background command. It matches \`idle\`, \`done\`, and \`blocked\`; \`idle\` and \`done\` mean read the handback, while \`blocked\` requires inspection. A wait with no state has died, not finished: re-arm it. Never wait on \`done\` alone. The wait is a convenience; the handback in the store is the return; a wait that outlives the handback (a lane with a background shell stays \`working\`) is resolved by reading the store, never by prompting the lane.
8. A delivery lane passes \`--candidate <commit or digest>\` with its DONE handback. It files the complete return with \`maestro handback file <dispatch-id> --status DONE --candidate "<commit or digest>" --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"\`; other lanes may omit the optional candidate. \`BLOCKED\`, \`DEPENDENCY_REQUEST\`, \`COUNCIL_REQUEST\`, and \`REOPEN_REQUEST\` also pass \`--request "<retry condition or requested action>"\`. A return packet is a claim; the Lead checks its evidence and decides whether the work item is complete. The lane must file exactly once, when the stop condition is met; a lane with a second stop point needs a second dispatch. Evidence that arrives later is \`maestro work note <work-id> "after h<id>: <evidence>"\` plus a \`[from peer]\` message; it never changes the filed handback.
9. Read cross-project attention with \`maestro brief\`. For Maestro commands outside this set, use the command's help.
10. After reviewing the handback, record the reading with \`maestro handback review <handback id> --note "<what you decided>"\`; only the opener may file it and it clears that handback's \`HANDBACK_UNREVIEWED\` finding, which closing the work or citing the handback in a later dispatch also clears. Then, after closing or re-dispatching the work, close the lane with \`herdr pane close <pane-id>\`, then \`herdr tab close <tab-id>\` once the \`lanes-<owner>-<work id>\` tab is empty. The pane stays only when the same lane takes the next dispatch. A \`[from lead]\` prompt never changes an assignment: a new objective or a new stop point is a new dispatch (step 4) on the same work item and the same pane, naming the predecessor handback in the objective. Transcripts persist on disk, so closing loses no evidence.
11. A finding returned in a handback is closed by that handback; it becomes a card only when it is the next thing the Lead will actually do.
12. Cross-examination, only when first views conflict or the risk warrants it: open a second generation of dispatches on the same work item, paste the other lanes' handbacks verbatim into each contract with one targeted question, and read the answers as handbacks (DONE with a CONFIRM claim, CHALLENGE, or REOPEN_REQUEST; CONFIRM is claim text, not a status). Lanes never prompt each other; the Lead reconciles.

No dispatch or work verb pushes a brief into a lane or calls Herdr. Herdr owns Peer-lane topology and delivery; the explicit supervised team lifecycle invokes TeamRuntime; Maestro owns the durable contract and evidence record.
`;

const lead = `# Handing owner intent to a repository Lead

The room relays owner intent to the repository Lead without taking project authority. Every team lifecycle command below runs from the Room store; the project store never owns a copy of team readiness.

1. When the owner states intent in the room, find the repository in \`registry\` and name the team that will own it. When the owner says fix or do, relay without asking whether to relay; ask the owner a question only for a real fork.
2. Read \`maestro team status <team> --json\`. It is snapshot only and cannot prove that a role or sensor is still live. If the team does not exist or is STOPPED, run \`maestro team open <team> --repo <repo> --operation <stable-id> --requested-by supervisor --wait-ms <bounded-ms> --json\`. TeamRuntime owns workspace, pane, agent, bootstrap prompt, and sensor creation, including the dedicated foreground \`maestro-team-sensor\`; the room never recreates any of those with direct Herdr startup commands.
3. Accept the open only when the same receipt says ACTIVE, READY, CLEAR, and OPERABLE. STARTING is a non-success with named missing postconditions. Before relaying new intent to an existing team, run \`maestro team health <team> --operation <stable-id> --requested-by supervisor-<team> --json\` for a fresh runtime inspection; \`maestro team await-ready <team> --operation <stable-id> --requested-by supervisor-<team> --wait-ms <bounded-ms> --json\` is the foreground bounded wait. DEGRADED, REVIEW_HOLD, DRAINING, and CLOSED are not healthy enough to accept consequential work.
4. Once the fresh verdict is OPERABLE, render the project's lesson view with \`maestro lesson render\`, so the Lead reads it fresh; the view is rendered from the room store and the repository's own, never hand-edited (d42). The only relay target is \`lead-<repo basename>\` whose cwd is the repository and whose current team generation TeamRuntime just proved. Never prompt a pane with any other name. Write \`[from supervisor][intent] <owner words verbatim>. You are the Lead of <repo>; this is owner intent relayed by the room; record it as work and choose your own route (d700); report to <record holder>; read ~/maestro/PROJECT/<repo basename>.md before your first card, it holds every correction already filed against this project.\` to a file and run \`herdr agent prompt lead-<repo basename> "$(cat <file>)"\`. \`<record holder>\` is \`supervisor-<team>\`: the Lead reads its return address from this prompt and never searches for it.
5. Use the Room-ledger lifecycle for every team intervention:
   - A supervised check is the boss-style one-shot inspection, not a persistent watcher: \`maestro team review spot-check <team> --operation <stable-id> --requested-by supervisor-<team> --question "<one question>" --window "<bounded evidence window>" --stop "<one verdict>" --json\`. Observer receives one bounded packet; resolve a raised hold only with \`maestro team review clear\` or \`maestro team review escalate\`, each with Supervisor rationale.
   - Advisor is on demand, never a baseline idle pane: \`maestro team advise <team> --operation <stable-id> --requested-by <requester> --decision <decision-id> --question "<one question>" --context <record> --stop-condition "<bounded return>" --timeout-ms <bounded-ms> --json\`. Only \`supervisor-<team>\` or the registered Lead may request it; TeamRuntime proves it, collects one recommendation, and closes it.
   - Repair only explicitly authorized resources with \`maestro team reconcile <team> --operation <stable-id> --requested-by supervisor-<team> --resource <resource> --json\`; valid resources are workspace, supervisor, lead, observer, and sensor. Reconcile never clears a review hold and there is no automatic restart.
   - Stop through \`maestro team stop <team> --operation <stable-id> --requested-by supervisor-<team> --wait-ms <bounded-ms> --json\`. STOPPING closes new gates first, keeps Observer and sensor live while work seats drain, closes the team Supervisor last from this external Room session, and reaches STOPPED only after fresh absence proof. Force is a separate explicit authorization with reason and evidence.
6. In the room store, run \`maestro work note <room-work-id> "handed intent to <repo>: <one-line summary>"\`.
7. Every card opened from a \`[from supervisor][intent]\` prompt reports once when it closes, whether or not it carries a room decision id: \`herdr agent prompt <record holder> "[from lead][done w<id> re <room record>] <candidate commit; one line on any deviation>"\`. A suite result in that line carries pass, fail and skip counts together and names the environment it ran in: a runner prints no skip line when the count is zero, and a test gated on an environment variable the lane's shell does not set is silently missing from its total, so a lane and the room report different numbers on the identical tree with nothing in either output saying why. A pass count alone is an incomplete claim, not a green (d7). \`<record holder>\` is \`supervisor-<team>\` whenever the pane sits in a team workspace. \`<room record>\` is whatever the relaying prompt named: \`d<room-id>\` for a decision, \`w<room-id>\` for a room work item when the prompt names no decision. Every store numbers decisions from d1, so a record id that crosses a store boundary in either direction is written with its store, \`room d41\` and \`<repo> d4\`, never a bare \`d41\`. Note that id on the card when you open it, so the close can name it. This is one prompt per closed card, after \`maestro work done\`, never before. \`maestro brief\` prints attention findings only, so without this the room cannot see a closure at all; the room still never polls the Lead. A report sent to the wrong holder is bounced, not absorbed: a supervisor answers a \`[from lead]\` prompt from a Lead it does not own with exactly one line, \`not my supervisor: send to supervisor-<team>\`, and it is neither verified nor recorded. Ownership comes from the generation's freshly inspected \`workspace_id\`, never cwd. \`herdr agent prompt <record holder>\` is the only channel this Lead has out of its pane, and the name is the one the opening prompt gave.
8. Never run \`maestro work add\` or any write in the project store, run \`maestro dispatch open\`, suggest topology in the prompt, or read the pane transcript. When the Lead needs a Supervisor decision, run \`maestro decision draft "<the choice>" --rationale "<why, options>" --work <id>\`, then \`herdr agent prompt <record holder> "[from lead][ask d<id>] <question>"\`. A non-decision question is a work note sent the same way. The room never runs \`herdr agent wait\` on a Lead: the store is the truth and the room's next prompt shows it. The record holder's reply is a prompt and the record (lock or supersede) is what the Lead acts on.

Every \`herdr agent prompt\` body this room sends is written to a file and passed as \`"$(cat <file>)"\`, for the reason in \`lane.md\` step 5: owner words repeated verbatim carry backticks, dollar signs and command names constantly, and a double-quoted argument is rescanned before delivery. Execution is the loud failure; the quiet one is that an unset variable expands to nothing and the Lead acts on a sentence whose verb was removed with nothing marking the gap.
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
    # The Supervisor is a Claude session under SLP, so d24's window applies to it
    # as much as to a lane. Only the window is fixed here: model names rot, so
    # model and effort stay the owner's, in OWNER.md, and both this kind and the
    # flags after -- are the owner's to edit together.
    herdr agent start supervisor --kind claude --pane "$root_pane_id" -- --autocompact 250000 >/dev/null
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

const observer = `# Observer contract

\`observer-<team>\` is a bounded judgment seat, not a watcher process. TeamRuntime starts the role and the dedicated foreground \`maestro-team-sensor\`, proves both against the current team generation, and stops both through the Room-ledger lifecycle. Starting, repairing, or replacing either resource is not Observer authority.

The sensor applies the fixed semantic catalog and sends one bounded evidence packet only after a threshold fires or \`supervisor-<team>\` requests a one-shot supervised check. A packet contains its id, packet capability, team and generation, rule and actor, bounded work, decision, authority and stop references, exact matched evidence, and a capped excerpt. Observer never receives a continuous whole-team transcript and never expands the packet by reading other panes or stores.

## On one packet

1. Verify that the packet names this team and generation and includes one packet id and capability. If any binding is absent or mismatched, submit nothing.
2. Judge only the cited evidence against the cited question, role boundary, and stop condition. Do not search for more context and do not turn suspicion into a general audit.
3. If the evidence warrants a consequential hold, run the following command from \`~/maestro\`, replacing every placeholder from the packet:

   \`maestro team review raise <team> --operation <stable-id> --requested-by observer-<team> --packet <packet-id> --capability <capability> --finding "<bounded finding>" --json\`

   This capability-bound command is the only scoped write Observer may make.
4. Submit at most one packet-bound verdict. If the evidence does not warrant REVIEW_REQUIRED, submit nothing. Never clear or escalate a hold; \`supervisor-<team>\` owns that decision.

## Authority boundary

Observer has no general store, work, dispatch, decision, reconcile, or runtime authority. It never starts or stops a role, changes an assignment, repairs a resource, accepts a candidate, or performs an external effect. It uses no command other than the capability-bound \`maestro team review raise\` call above. Dedupe belongs to the Room ledger, so unchanged evidence cannot create a second delivery; the same rule may fire again only on new evidence.
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
    ["observer.md", observer],
    ["shellrc", shellrc],
  ] as const) {
    await writeFile(join(room, name), content);
  }
  await rm(join(room, "observer-watch.sh"), { force: true });
  try {
    await writeFile(join(room, "OWNER.md"), owner, { flag: "wx" });
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "EEXIST") throw error;
  }
  await chmod(join(room, "OWNER.md"), 0o600);
  return room;
}
