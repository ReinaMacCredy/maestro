# Changelog

All notable changes to Maestro are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased history

The 0.107.x series was the Rust implementation. Version 0.108.0 begins the
TypeScript-on-Bun line and continues the existing version sequence.

## [Unreleased]

### Fixed

- `maestro decision draft <draft-id> --rationale <why>` (also `--dissent`,
  `--review-at`, `--needs-owner`) now edits that field on the existing draft
  and keeps its text; a locked target answers `LOCKED_DECISION`, and a bare id
  with no field still answers `MISSING_ARGUMENT` (w587).

## [0.116.0] - 2026-09-02

### Added

- After `work return`, `work accept`, and `work note --rework` commit, the
  binary prompts the counterpart's pane with one line,
  `[from <role>][<id> <STATE>] <summary>; read: maestro status <id>`, so a
  Team Supervisor or Lead waiting on a hand-off no longer polls the store for
  it. The reviewer is derived from the assignee's role, never from
  `created_by`; accepting Hub-created work and a normal stop also notify the
  Herdr agent named `supervisor`. A refused prompt only warns and the store
  stays the truth (d753).
- The Team Supervisor may pass `team stop --reason <text>`. The reason rides
  the stop grant into the STOP lifecycle row, Hub status prints
  `<team> g<n> STOPPED (supervisor): <reason>`, and JSON exposes `stop`. Hub
  `--reason` still requires `--emergency` (d760).
- `maestro search --limit <n>` bounds the result count; the default is five,
  and a truncated listing says how many more there are (d754).
- `team start` and `work add --to` print their phases on stderr, and the pack
  states that both block until the role pane acknowledges (d757).

### Changed

- `maestro status` hides dead sessions at read time: the text lists live
  sessions and only those dead sessions that still hold work, closes with a
  count of the hidden ones, and `--all` lists everything; JSON carries
  `hiddenDead`. `maestro status <work-id>` outside an active SLP v2 role pane
  refuses and points at `work show`. In this repo the default text went from
  143 lines to 7 (d754).
- In-team `maestro status` text is structured: a header naming team,
  generation, role, name, pane, and missing panes; one line per non-DONE item
  sorted OPEN, ACTIVE, RETURNED with `*` on the items the caller may act on;
  DONE collapsed into a count that `--all` expands; and the generation's
  decision ids. `maestro status <work-id>` prints six lines ending in a
  `next:` line that names what the caller may run or whom it waits on. A Peer
  sees only its own items. `--json` output is unchanged (d758).
- `team start` against a RUNNING generation repairs only the missing roles. A
  role whose recorded pane is still attached under the same instance id with a
  recorded acknowledgement gets no contract prompt and no second
  `SLP_ROLE_READY`, and the START lifecycle row in both stores is rewritten to
  the current pane ids with the revision advanced so the audit snapshot
  matches the role table. The same rule covers `work add --to` against a live
  Peer (d759).

### Fixed

- `team start` and `work add --to` poll the role pane with non-scrolling
  visible reads inside a 30 s window instead of reading once right after the
  prompt returns; a pane that never answers ends on one unwrapped read and the
  failure carries the pane tail. Runtime errors are typed (`SLP_RUNTIME`,
  `ROLE_ACKNOWLEDGEMENT_MISMATCH`, `TRUST_DIALOG`, `AGENT_BLOCKED`) with Herdr
  evidence, and an agent blocked on the claude or codex directory trust dialog
  is named as such with the directory to trust (d755, d756).
- Acknowledgement matching strips display markers in front of
  `SLP_ROLE_READY`, reassembles an acknowledgement the pane wrapped across
  lines, and waits for an agent Herdr reports as not ready instead of failing
  the start on the first prompt.
- `maestro install` removing a retired SLP mirror block that ends a file now
  collapses the trailing newlines to one and leaves an otherwise empty file
  empty.

## [0.115.0] - 2026-09-01

### Added

- SLP v2 provides one direct supervised-team architecture with four roles,
  exactly nine public operations, one canonical Hub-owned Workspace Pack, and
  a content-addressed generation snapshot copied into each project. Start
  provisions an exact Team Supervisor and Lead contract, Peers are opened only
  by assigned work, and each role must acknowledge its team, generation,
  instance, pack, brief, and challenge before it receives project authority.
- Project-owned work uses the single `OPEN -> ACTIVE -> RETURNED -> DONE`
  lifecycle. Objectives and acceptance contracts are immutable; reviewer-owned
  `work note --rework` grants one retake of one return revision, while emergency
  stop records explicit abandonment metadata without inventing a fifth state.
- An optional foreground Watch Pane labels currently available raw role output.
  It is a non-agent reader with no prompt, gate, store mutation, or decision
  authority, and its runtime-only transcript is deleted when the team stops.
- Start and stop now carry durable internal lifecycle phases, revisions, exact
  role bindings, and recovery metadata so interrupted external Herdr effects
  can converge on retry without exposing more than public `RUNNING` and
  `STOPPED` states.

### Changed

- The earlier layered team lifecycle is a hard cut to SLP v2. Observer,
  Advisor, sensor, review, reconcile, packet, and background-agent SLP roles are
  removed; their previous records remain readable legacy history. Team
  Supervisor, Lead, and Peers communicate directly, while the supported Hub
  flow reaches a team through its Team Supervisor.
- The SLP contract now states its actual boundary: it is a cooperative-agent
  protocol, not a shell security sandbox. Hub operations are scoped by the Hub
  room and project role operations by the current generation's stored Herdr
  pane binding; native commands, administrative Maestro commands, and direct
  Herdr calls remain outside that enforcement boundary.

### Fixed

- SLP mutations and their minimal activity records commit atomically, including
  start, stop, decisions, work transitions, and rework grants. Audit insertion
  failures and SQLite contention return structured errors without partial
  authoritative state.
- Concurrent identical starts converge on one generation, changed contracts
  are rejected, work and decision identifiers do not collide, start repair and
  stop are mutually fenced, and linked Git worktrees retain distinct project,
  runtime, role, work, decision, and shutdown boundaries.

## [0.114.0] - 2026-08-29

### Added

- `maestro handback file --lessons <lesson id>@<store path>` repeats and names
  the lessons a return answers. Each id is resolved against the store it names
  when the handback is filed: an id absent from a readable store refuses, and a
  store that cannot be read warns and files. `lesson process` still writes only
  to the store it runs in, so the room reads the field and runs the verb in its
  own cwd instead of retyping commit shas out of a relay.

### Changed

- The observer's sensor is opened by the room in a plain shell pane of its own,
  split beside the observer and started with `herdr pane run`, and the observer
  is no longer told to background it from its own shell. A background process
  started from a harness's tool command does not outlive that command: the first
  live sensor reported a pid that was gone minutes later, with no state
  directory and no wait armed, while the same script in a sibling shell pane had
  both inside 25 seconds. `observer.md` and `lead.md` carry the new shape, and
  that template is now stated to be the only start message the room sends an
  observer, since a hand-written handoff in its place left one watching for
  three minutes and then silent (room d60).
- `observer-watch.sh` arms a wait only for a working pane, so the sensor fires
  on the transition out of working and once per transition. `herdr agent wait`
  on an agent already idle, done or blocked returns at once, so arming every
  pane re-fired the whole settled set every cycle: events grew by one line per
  settled pane per cycle and a settled pane whose tail held a matching line woke
  the model every 15 seconds until that tail scrolled away (room d60).
- The `consult-<repo basename>` rung is retired from the slp recipe and the
  roles page. Its read-only investigator duty is `advisor-<team>`'s, and the
  help ladder is peers, then the Lead, then `advisor-<team>`, then
  `supervisor-<team>`, with no seat between the Lead and the advisor. The rung
  sat on the same model and the same read-only grant as the seats on either side
  of it, so it added a hop and nothing else, and the team that had one skipped
  it on its own: every difficulty went from the Lead to the record holder
  directly (room d58). The ruling was made for one team and is generalised to
  the recipe here on the room's stated assumption, which the owner may override.
- `maestro decision draft` refuses a lone positional that names an existing
  decision, instead of creating a second decision whose text is that id, and
  refuses `--supersedes`, `--parent` and `--work` on an edit, which the edit
  statement parsed and dropped without a word. A supersession could be typed,
  accepted and lost; the two failures compound, so both close together.
- `maestro update` refuses a source checkout that has a remote but sits on a
  branch no remote has published, with `UPDATE_SOURCE_UNPUBLISHED` naming the
  branch and the next command; lanes branch inside the checkout the runtime is
  installed from, so an unpublished branch there is unreviewed code. A checkout
  with no remote is unaffected.
- The drift line names the branch it read the source commit from and the number
  of commits no remote holds, so a line that means the runtime is behind can no
  longer read the same as a line that means the runtime would be built from a
  lane branch.
- The room's binding in `IDENTITY.md` now states its evidence rule for every
  claim the room makes, in a brief or a report as much as in a gate decision,
  and names the two surfaces that read as evidence and are not: a line that
  arrived in context rather than from a command just run, re-checked by running
  the verb that emits it, and a listing truncated for display, which proves at
  least N and never exactly N. A stale drift advisory read at session start was
  nearly reported to a Lead as a live hazard, and the length of a `head -5`
  listing was quoted in a brief as a count of five where the real number was
  fifty-eight. The external-effect gate's own definition of verified evidence
  defers to that rule instead of restating it, and keeps the worked example it
  carries, so the binding defines the term once (room l7: room w9, room d53;
  room l10: room w6).
- A brief body is now sent through a file, `herdr agent prompt <name>
  "$(cat <file>)"`, in `lane.md` step 5 and for every prompt the room itself
  sends in `lead.md`. A double-quoted argument is still scanned by the sending
  shell, so a backtick or a dollar-parens in text describing a command runs on
  the sender's machine, and an unset variable expands to nothing, leaving a
  brief that arrives complete-looking with its verb removed. That is what
  happened to a relayed note, whose literal command text vanished before it
  reached the room. The rule governs literal or owner-supplied text and not a
  script expanding its own variables on purpose, and the send forms in those
  steps are file-backed themselves, so no example teaches the form the rule
  forbids (room l8: room w6).
- A suite result in a Lead's closure line now carries pass, fail and skip
  counts together and names the environment it ran in, in `lead.md` rule 6, and a pass count alone
  is an incomplete claim rather than a green. A runner prints no skip line when
  the count is zero, so two environment-gated tests a lane's shell could not run
  were silently missing from its total: the lane reported 490 pass 2 skip on the
  tree the room knew as 492 pass 0 skip, and it took three messages to establish
  that the gap was environmental and not a fork divergence (room l9: room w6).

## [0.113.1] - 2026-08-29

The patch that retires the run-update-twice rule and puts the loop on the site.
One `maestro update` now brings the room fully onto the commit it installs, the
self-improvement loop has a page a reader can land on, and the first improver
run turns five corrections from a live team into four doctrine edits.

### Added

- Documentation site: a Self-improvement guide covering the loop end to end for
  a reader, from filing a lesson through the threshold, the improver's smallest
  edit, the golden replay gate, the challenge lane, and the rendered per-project
  view. The loop shipped as recipe text, which reaches a session and not a
  reader of the site. Reachable from the sidebar and cross-linked with the CLI
  reference, Attention and brief, SLP scenarios, and Recipes, skills, and
  plugins.

### Changed

- The room's external-effect gate now says where a claim is checked: verified
  evidence means every claim in the locked decision checked at the surface it
  names, and a docs or site claim is verified by opening the reader-facing page
  a reader would land on, not by mentions inside existing pages. The v0.113.0
  release passed the gate as "docs and site refreshed" while the site had no
  page on the self-improvement loop, and the owner found the gap (l2: w9, w10,
  d47).
- A team seat now reads its report target as `supervisor-<team>`. `lead.md` rule
  6 spelled the command with the bare name `supervisor`, which is the room, and
  the recipe's team table said what each seat may write but not where it
  reports; in a new team workspace `advisor-cmux`, `observer-cmux` and
  `consult-cmux` all sent their ready straight to the room. Rule 6 now spells
  the command with `<record holder>` and says which name that resolves to, and
  the team table says only `supervisor-<team>` has a channel out of the
  workspace (l3: w6, d28, d30, d36).
- A BLOCKED handback now records the negative knowledge: its claim names the
  mechanism that failed and the alternatives that attempt killed, and its proof
  names what falsified each one, beside the retry condition it already carried.
  On w9 the mechanism-level cause of a blocked return lived only in a pane, and
  the outgoing Lead had to spell out four dead mechanisms in a stand-down
  message or they would have closed with it (l4: w6, d41).
- A record id that crosses a store boundary now names its store, `room d41`
  rather than `d41`, in `lead.md` rule 6 and in the retry condition of a
  handback. Every store numbers decisions from d1: the room cited d41 at a team
  whose store held d1 to d4, `maestro decision show d41` returned NOT_FOUND
  there, and the team could neither read nor correct the decision blocking its
  handback (l5: w6, d41, d51).

### Fixed

- One `maestro update` now materializes the managed skills shipped by the commit
  it installs. `skillNames` and the skill sources live in a module the running
  process imported before the runtime swap, so an update that installed a
  release adding a skill wrote the outgoing release's list, and only a second
  pass wrote the new one; the room saw it at v0.113.0, where `maestro-improve`
  appeared on the second update. Skills are materialized from the post-swap
  runtime in a fresh process, the same way the room templates already were.
- The slp recipe and the `maestro-improve` skill name the golden scenarios
  `tests/scenarios/<name>.script`, the placeholder the site and the CHANGELOG
  already used. They said `<case>`, which is only half the directory:
  `lesson-loop.script` sits beside `case-1-atomic-fix.script`.
- `maestro lesson render` prints a repository it could not read before the
  summary that repository is missing from, and the line names the command that
  shows why: `cd <repo> && maestro lesson list --all`. The warning was right to
  be loud but landed under the summary and said nothing about what to do, so a
  store left out of the view read as a footnote to it.

## [0.113.0] - 2026-08-29

The self-improvement release. A correction stops being a remark in a transcript
and becomes a record: whoever makes one files it where it happened, an improver
lane reads the pile on a threshold rather than reacting to each one, and a
golden replay decides whether the doctrine edit it proposes was an improvement.

### Added

- Lessons (room d40, relay w9, card w550, decisions d722 and d723).
  `maestro lesson file` records a correction as a store row naming the doctrine
  it corrects, what happened, what was expected instead, why that expectation is
  the right one, and the work, handback or decision ids that evidence it.
  `lesson list` shows what is still pending and `--all` includes what is not;
  `lesson show` reads one; `lesson process` marks it with the commit that
  carries its edit, or with `--answer`, the reason it produced none. Nothing
  deletes a lesson. Evidence ids are text rather than foreign keys, so a
  repository lesson may cite room ids that do not exist in its store (d723), and
  the project tag defaults to the store's own project, the registry name the
  room renders its per-project view from.
- `LESSONS_PENDING`, an attention finding (room d42, card w551, decision d724).
  Five pending lessons for a project, or seven days since that project's last
  improver run, raise it; before any run the oldest pending lesson starts the
  clock. It is computed at read time from store state like every other detector,
  so `maestro brief` carries it across registered projects with nothing
  resident, and it groups by project tag rather than by store, because the room
  relays "run the improver" to the Lead of the doctrine those lessons target and
  a store-wide count names no Lead.
- `maestro lesson render` (room d42, card w552, decision d725), which writes
  `~/maestro/PROJECT/<project>.md`, one file per project tag, from the store it
  runs in plus every store in `~/maestro/registry`, each read through a
  read-only child process rather than by opening another store's database.
  Pending and processed lessons both appear, so a new team inherits the whole
  record. Like `registry` it is a rendered view and is never hand-edited. The
  room runs it before it hands intent to a Lead, and the start prompt now names
  that path for the Lead to read before its first card.
- `maestro-improve`, a fifth managed method skill (room d44, card w553). The
  improver lane groups pending lessons by target, proposes the smallest edit per
  group as a commit carrying their evidence ids, marks each lesson processed by
  that commit or answered with the reason it was rejected, and stops at the
  handback for the Lead. Its target catalogue is a reference file, so the skill
  itself stays short.
- A scenario golden harness (room d43, card w554, decision d726). Each SLP
  scenario is a script of maestro commands in `tests/scenarios/<name>.script`
  with the transcript it produced beside it in `<name>.golden`, replayed against
  a fresh store by `tests/scenario-golden.test.ts`. A line prefixed with
  `@<session>` runs as that lane, so one script holds both the Lead and the
  Peer; timestamps, paths and pids are normalised, so a diff in a golden is a
  change in behaviour. The four scripts cover the atomic fix, the delivery lane
  through dispatch, confirm, handback, attention and review, the sealed decision
  council, and the lesson loop itself. Scenarios are data: the improver adds one
  without writing test code, and an improver edit is accepted only when the
  replay still matches, or matches the change a lesson asked for, re-recorded
  with `MAESTRO_GOLDEN_UPDATE=1` in the same commit as the edit.

### Changed

- `maestro work done` takes the lease itself when no session holds the item,
  completing in one command (room d700 relay w8, decisions d720 and d721). The
  implicit claim runs the same blocker check and `work.start` gate chain as
  `maestro work start`, so a no-write lane still cannot take the lease through
  completion, and a lease another live session holds still raises
  `LEASE_HELD`. No `work.start` event is written for the implicit claim; the
  `work.done` event carries `claimedOnDone`. When a previous holder lost the
  lease, the completion text names that holder and the liveness reason the
  removed `LEASE_REQUIRED` error used to carry.
- The SLP recipe carries the self-improvement loop it all sits in (d40, d42,
  d43, d44): who may file a lesson and why a Peer's finding travels through its
  handback and the Lead, the threshold that starts a run rather than a
  correction firing one, the delivery lane on the strong rung followed by a
  challenge lane on the diverse rung, and the golden replay that gates an edit.

### Fixed

- The room's Lead start prompt named the rendered project view inside backticks,
  which a shell would have run as command substitution instead of passing as
  text.
- `.claude/`, `.codex/` and `.idea/` are ignored rather than tracked. The
  installer owns the harness wiring and the IDE file is per-checkout state; both
  were committed by accident with the lesson view.

## [0.112.0] - 2026-08-29

The team release. SLP gains a team dimension: a team is one Herdr workspace,
with one record holder, two supports that hold no records, and exactly one
channel back to the room.

### Added

- Teams (room d28, d29). A team is one Herdr workspace; a session reads its
  role from its agent name prefix and its team from the workspace it sits in,
  never from cwd. A team holds exactly one record holder, `supervisor-<team>`,
  beside `advisor-<team>` (read-only counsel for when it is stuck or the owner
  is away) and `observer-<team>`. One team cwd maps to exactly one workspace:
  an opener reuses a match from `herdr workspace list` before creating one,
  because a second workspace on the same cwd splits a team in half without
  saying so. The Supervisor room is its own workspace and opens no agent in it.
- `observer-<team>` and its watcher (room d33). `~/maestro/observer-watch.sh`
  is scaffolded executable beside the room templates and runs in the observer's
  own pane: it arms one `herdr agent wait` per team agent plus a five-minute
  sweep over the working ones, reads tails with
  `herdr agent read --source recent-unwrapped`, and prompts the model only when
  a tail matches a countable trigger. Judgment stays with the model, which
  reads further, checks the store under `MAESTRO_READ_ONLY=1`, and either sends
  one `[from observer][suspected]` line to the member that drifted or stays
  silent, keeping a once-per-issue ledger outside every store. The watcher is
  not a maestro process: no verb starts it, it opens no store, and it dies with
  its pane, so the A1 no-daemon gate still holds.
- `~/maestro/observer.md`, the room's template for opening an observer.
- The report target is a parameter (d719). A Lead reports its closed card to
  the record holder the relaying prompt named, so a Lead inside a team reports
  to that team's `supervisor-<team>` rather than past it to the room. The Lead
  still never searches for its supervisor.

### Changed

- The Supervisor holds the owner's authority in full (room d37). The three
  `IDENTITY.md` binding lines that read `none` now carry write authority
  (the owner's, external effects included), acceptance authority (the owner's,
  at the owner boundary; technical acceptance stays with the Lead unless the
  room takes a lane over), and a standing recovery lease in any team: the room
  may freeze work, override or supersede a team decision, redirect or replace a
  `supervisor-<team>` or a Lead, and order a correction. A code correction
  still goes through that team's Lead and its lanes. The binding gains an
  external-effect gate: a push, tag, release, publish, deploy, `maestro update`,
  remote, deletion, or machine-config change runs only after a locked room
  decision names the exact candidate and the verified evidence, never straight
  from a Lead's prompt, with the command and its output recorded (room d6).
- One prompt crosses a workspace boundary upward (room d36):
  `supervisor-<team>` reports to the room. Leads, advisors, observers and peers
  never prompt the room, and the room reaches a team through its
  `supervisor-<team>`, except for a Lead it opened and still owns.
- A misrouted report fails closed (room d35). A supervisor answers a
  `[from lead]` prompt from a Lead it does not own with exactly
  `not my supervisor: send to supervisor-<team>` and neither verifies nor
  records it, because absorbing it would leave that team's record holder never
  learning the work closed. Ownership is read from the Lead's `workspace_id`,
  never from cwd.
- A Lead reports every card opened from a `[from supervisor][intent]` prompt,
  whether or not it carries a room decision id, keyed on whichever record the
  prompt named. A relay that names no decision previously matched nothing, so
  the card closed unreported. `herdr agent prompt supervisor` is stated as the
  only channel to the room, so a Lead never hunts for the room with agent or
  dispatch listings.
- `--target-session` documentation names its value in the flag help, `cli.md`,
  and the role contract: the accepting session's harness session id, never the
  Herdr agent name. No validation added, because session ids are arbitrary
  strings.
- README, the roles and lanes concept pages, and the SLP scenarios guide carry
  the team model; the enforcement-claim registry records team membership,
  observer read scope, one workspace per team cwd, the clean room workspace,
  the upward channel, the misrouted report, the observer sensor, and the
  external-effect gate as soft-audited.

### Fixed

- `maestro update` scaffolds the room from the runtime it just swapped in. The
  process had imported `room.ts` before the swap, so a first update installed
  the new commit while rewriting `~/maestro` from the outgoing release and only
  a second update materialized the new text. Scaffolding now runs in a fresh
  process; a cache-busting import query does not work here, because Bun keys
  its module cache on the resolved path.
- `TARGET_MISMATCH` names its recovery path. `confirm` cannot repair a dispatch
  nobody can accept, so the message now names both exits, cancel and reopen
  with the accepting session's id or open pane-bound without the flag, and
  ships the same string as `fix` in the error data.

## [0.111.1] - 2026-08-29

### Changed

- Every Claude agent SLP starts now carries `--autocompact 250000` (owner
  order, room d24): the Model table's Claude cells, the Claude command strings
  in the lane and lead templates, and the `hm` shell function, which previously
  started the Supervisor with no flags at all. Codex command strings are
  unchanged, since Codex has no equivalent flag. `hm` takes the window but not
  a model: model names live in `OWNER.md`, and the `--kind` plus the flags
  after `--` are named as one owner edit point so switching to `--kind codex`
  does not leave Claude flags behind.

## [0.111.0] - 2026-08-29

The public-adoption release (d8). Maestro is now licensed, has a security
policy, and no longer executes code a repository carries.

### Added

- MIT `LICENSE` (owner choice, room d16), `SECURITY.md` with a private
  reporting channel and the boundaries a report is judged against, and
  `CONTRIBUTING.md` naming the bun-only toolchain, the test-first rule, and the
  A1-A3 gates CI enforces.
- Plugin trust boundary (d712, d713). A global or repository plugin executes
  only when `~/.maestro/trust.json` holds a grant matching its canonical path
  and a sha256 over every file in the artifact. `maestro plugin trust` and
  `plugin untrust` manage grants; `plugin add` trusts the bytes it just cloned
  and no longer imports them; `plugin new` scaffolds untrusted. An untrusted
  plugin is named from the filesystem and never imported, so `plugin list`
  executes nothing.
- `maestro handback review <h-id> --note <text>` (d17) records that the opener
  read a return packet and clears its `HANDBACK_UNREVIEWED` finding. Only the
  session that opened the dispatch may file it, and a second review is a no-op.
- PR CI over the trees the root suite never compiled: the desktop TypeScript
  build and tests, the desktop Rust crate on macOS, and the documentation site.
- A store records its schema generation in `PRAGMA user_version`, and a Maestro
  older than the store refuses every command with `STORE_TOO_NEW` rather than
  writing into a shape it does not know.

### Changed

- `scripts/install.sh` installs the newest pushed release tag by version order
  onto a `maestro-release` branch instead of the tip of `main` (d714).
  `MAESTRO_REF` remains the development escape hatch, and `maestro update`
  moves a pinned checkout tag to tag while every other branch keeps its
  upstream fast-forward. Pinning buys a release and a reproducible install; it
  is not tamper-proofing, because `install.sh` itself is fetched from `main`.
- The desktop data layer runs every verb under `MAESTRO_READ_ONLY=1`, so
  watching a store no longer writes a session row, heartbeats liveness, or
  loads the watched repository's plugins.
- A Lead reports a closed card carrying relayed room intent with one
  `[from lead][done w<id> re d<room-id>]` prompt (d22); `maestro brief` prints
  attention findings only, so a closure was otherwise invisible to the room.
- The `HANDBACK_UNREVIEWED` packet's smallest action is the review verb on
  every branch; the status-specific next move moved into its question.
- A lane's cwd decides which store it reads, so the lane procedure states the
  room lane's cwd and makes a lane compare its stored contract against the one
  it was sent before accepting (d717).

### Fixed

- `LEAD_COLLISION` missed a holder pair where one card was parentless and the
  other a child, so two sessions could hold active work in one repository with
  `maestro attention` reporting nothing (d21). A pair sharing a parent stays
  `SCOPE_COLLISION` alone, so one incident is still one packet.
- `STALLED_LEASE` raised nothing when the holder session was dead, leaving an
  abandoned card active forever; a dead holder now raises regardless of
  freshness, as an unreturned dispatch already did.
- `plugin list` reported an untrusted plugin twice, once as untrusted and again
  as a missing source.
- Tests 47 and 308 exceeded Bun's 5s default on an idle machine and now carry
  explicit timeouts.

## [0.110.0] - 2026-08-28

### Added

- `policy-card-budget` (d703): a removable policy on the new `work.add` gate
  that refuses a new card while the store already holds `limit` (default 3)
  open cards with no live holder and no accepted dispatch; enabled here.
  lane.md: a finding returned in a handback is closed by that handback and
  becomes a card only when it is the next thing the Lead will do.
- Declared councils (d706): `dispatch open --council-members <n>` and
  `--council-anchor <id>` record membership instead of inferring it from
  timing; the seal is a property of the declared council, whatever order the
  lanes open and return. One handback per dispatch is now a store constraint,
  and only the session that opened a dispatch may cancel it.
- Enforcement registry: the boundary table in `concepts/lanes.md` gives every
  row an id and cites the test that attacks it, or says `soft-audited`;
  meta-tests refuse a row without an attacking test and an SLP mechanism
  token that no row claims.
- Roles travel in pane names (d709): the room finds a repository Lead only as
  the Herdr agent it started as `lead-<repo basename>`, the Lead starts each
  lane as `peer-<dispatch id>` after `dispatch open --pane`, a peer-named
  session refuses any prompt that is not its stored contract, and the hook
  brief prints a store-derived `role:` line on every prompt (open dispatch
  ids, closed ones as a count; a returned holder is still a peer). The
  repository mirror block names the Lead the same way.
- The Supervisor room is a recorded store fact: the installer writes
  `meta.kind=room` into the room store; `doctor` applies the room contract
  there (files, registry, hooks, deny list, store) and never prescribes
  `maestro install`; `install` and `uninstall` refuse with `INSTALL_IN_ROOM`;
  the room hook brief carries a Supervisor intake line; the room `AGENTS.md`
  names the repository-only verbs.
- SLP Model reference (d711, guidance only): four rungs (cheap, strong,
  diverse, lead) with dated per-harness examples the owner keeps current in
  `OWNER.md`, a thinking level per lane, and the effort flags to pass; the
  Lead picks a lane's model the way it picks a sub-agent's, the room picks the
  Lead's model, nothing records or enforces the choice.

### Changed

- Session identity (d705) is never adopted from an incidental signal: without
  an explicit id a process mints `anon-<uuid>`; the pid stays liveness-only.
- Read-only opens fall back to a `query_only` handle for a cleanly closed WAL
  store, so declared-pure verbs answer the same with or without the sidecars.
- The Peer `PreToolUse` deny now covers a session on any dispatch row in any
  state, matching the role line.
- lane.md: a lane never talks to the Lead through the terminal; its returns
  are the handback and `--request`.

### Fixed

- `work cancel` on a parent cascades to its open or active descendants in one
  transaction with a prefixed reason and one event per row.
- `work add` records the session, so a session whose only verb was `work add`
  appears in `status --live`.
- `dispatch confirm` accepts a targeted dispatch already held by the named
  session instead of failing with `CLAIM_MISMATCH`.
- The `role:` line no longer lists every dispatch the Lead ever opened.

## [0.109.0] - 2026-08-28

### Added

- `scripts/install.sh`: one-command source install
  (`curl -fsSL .../scripts/install.sh | sh`) that clones the repository into
  `~/.maestro/source` and runs the installer from it, so `maestro update`
  keeps following that checkout; `install.sh --help`; a Bun 1.4 floor.
- Expanded SLP with a no-write shadow lane, explicit `COUNCIL_REQUEST`
  handbacks, sealed council cross-examination, one Lead per scope, and
  searchable Lead handoff receipts; `bundle open` refuses a successor whose
  handoff packet still holds placeholders.
- Bound the single Supervisor's scope and authority in `IDENTITY.md`, deny
  Claude sub-agent tools in the Supervisor room, and seed `OWNER.md` with the
  interview questions the room asks on its first session.
- `~/maestro/lead.md`: the Supervisor hands owner intent to a repository Lead
  through Herdr, prompting a live Lead or starting one, without writing to the
  project store.
- The intake contract (d700): the Lead scores the ROI questions from the
  repository and the prompt, asks at most one owner-boundary question, and
  announces the score, the route, the deciding fact, and the adjacent route
  not taken with the phrase that switches to it.
- Attention kinds `LEAD_COLLISION`, `HUMAN_DECISION_REQUIRED` (from
  `decision draft --needs-owner`), and `DECISION_REVIEW_DUE` (from
  `decision lock --review-at`); `SCOPE_COLLISION` now also compares the
  declared scopes of open delivery dispatches across work items.
- Typed return details: `handback file --request` is required for
  `BLOCKED`, `DEPENDENCY_REQUEST`, `COUNCIL_REQUEST`, and `REOPEN_REQUEST`,
  and `HANDBACK_UNREVIEWED` names the recovery path per status; optional
  `--candidate` on `handback file` and `work done`; `--dissent` on decisions.
- `dispatch confirm`: an untargeted dispatch accept records a claim that only
  the opener can confirm before the lane can work or return.
- A Claude `PreToolUse` hook denies sub-agents to a session that holds an
  accepted open dispatch; Leads and Codex are unaffected.
- `install` and `update` warn, and treat unreadable registered repositories as
  unsafe, before replacing the shared runtime.
- Method skills carry a `review-date`; `maestro doctor` reports overdue ones.
- Docs: an owner-seat SLP scenarios guide, an enforced-versus-soft-audited
  boundary table, build-time Mermaid diagrams with no runtime CDN dependency.

### Changed

- Cross-role decisions now use Herdr for transport and Maestro decisions or
  work notes as the durable record; one dispatch ends with exactly one
  handback (d697).
- Repeated failures now route Peer-held work to the Lead and Lead-held work to
  the Supervisor.
- `decision draft --work` warns when that work's council is still sealed; the
  recipe forbids storing the Lead's view before first views return.
- The managed instruction block names the repository's own `AGENTS.md` and
  `CLAUDE.md` as its Workspace Protocol.
- Documented quickfix, Light, and Full method tiers with explicit handoff and
  evidence boundaries.

### Fixed

- Sealed council returns are hidden from `handback list --json` and attention;
  a no-write lane holder cannot take the work lease; `--atomic-reason` is only
  written when `work start` succeeds; `update` treats untracked `src/` and
  `bin/` files as dirt; the doctor reads Codex hook trust from
  `~/.codex/config.toml`.

## [0.108.0] - 2026-08-27

### Added

- Rebuilt Maestro in TypeScript on Bun around a mechanism-only kernel, removable
  verb and policy plugins, and prompt-first Markdown recipes and skills.
- Added durable work, decisions, bundles, dispatches, handbacks, councils,
  attention packets, observer mode, a cross-repository brief, and the
  Supervisor, Lead, and Peer role model.
- Added source-checkout installation, fast-forward-only updates, read-only
  diagnostics, repository uninstall, a private Supervisor room, and four
  installed method skills.
- Added a bounded stdio MCP interface for finding and running Maestro verbs.
- Added read-only import of Rust-era stores plus idempotent promotion of legacy
  work, decisions, supersession links, receipts, and archived snapshots.

### Changed

- Coordination lanes now use Herdr panes for agent lifecycle and wake-up while
  Maestro remains the durable contract and evidence record.
- Attention is computed when read instead of delivered through a daemon or
  mailbox. Returned but unreviewed handbacks now receive immediate attention.
- The Rust-era stores are preserved under `legacy/rust/` for import and
  provenance.

### Fixed

- Serialized store transitions and ID allocation so concurrent work, decision,
  and dispatch operations cannot lose leases, collide IDs, or leak raw SQLite
  errors.
- Scoped council membership to the current concurrent generation and applied
  decision supersession only when the replacement locks.
- Rejected blank required arguments and invalid lane names with actionable
  errors.
- Hardened installer source trust, symlink boundaries, configuration writes,
  file permissions, rollback behavior, and runtime restamping.
- Made observer search fail closed when its index cannot be refreshed.
- Validated malformed MCP frames without terminating the server.
- Preserved native chronology and evidence when Rust data is promoted, including
  repeated promotion and missing-receipt handling.
- Corrected lane session resolution, handback status guidance, and command
  contracts in generated documentation.
