import { expect, test } from "bun:test";
import { join } from "node:path";
import { builtInPlugins } from "../src/plugins/index.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { skillNames } from "../src/plugins/skills.ts";
import { runCli, withFixture } from "./helpers.ts";

// room d134 (d892): the pack carries two shared blocks, the default one every
// seat reads and the Peer's.
function block(pack: string, marker: string): string {
  return (new RegExp(`<!-- slp:${marker}:begin -->([\\s\\S]*?)<!-- slp:${marker}:end -->`)
    .exec(pack)?.[1] ?? "").trim();
}

const slpOperations = [
  "maestro team start",
  "maestro team stop",
  "maestro status [work-id]",
  "maestro work add",
  "maestro work take",
  "maestro work note",
  "maestro work return",
  "maestro work accept",
  "maestro decide",
] as const;

test("308 [lint] the canonical Workspace Pack exposes only the locked SLP v2 contract", async () => {
  const pack = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"))
    .text();
  const publicSurface = (
    pack.match(/The public SLP surface is exactly:\n\n```text\n([\s\S]*?)\n```/)?.[1] ?? ""
  ).split("\n");
  expect(publicSurface).toEqual([...slpOperations]);
  expect(pack).toContain("OPEN -> ACTIVE -> RETURNED -> DONE");

  // Hub d91/d98: the seat mandates live in the shipped profiles, not the pack.
  // room d134 (d891, d894): the Peer mandate names no rung and no verb it
  // cannot run; its surface is listed in the pack's Peer block instead.
  const peer = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "profiles", "peer.md"),
  ).text();
  expect(pack).not.toContain("slp:role:peer:begin");
  for (const allowed of ["the scope it names is yours", "work note", "the blocker named", "Judgment inside the scope is yours"]) {
    expect(peer).toContain(allowed);
  }
  for (const forbidden of ["team start", "team stop", "accept Peer", "decide technical", "Role: Peer", "the Lead", "Team Supervisor", "SLP"]) {
    expect(peer).not.toContain(forbidden);
  }
  const peerBlock = pack.match(/<!-- slp:shared:peer:begin -->([\s\S]*?)<!-- slp:shared:peer:end -->/)?.[1] ?? "";
  expect(peerBlock).not.toBe("");
  for (const surface of ["maestro status", "maestro work take", "maestro work note", "maestro work return"]) {
    expect(peerBlock).toContain(surface);
  }
  // d894: the refusal is a verb it does not run, never a sentence about rank.
  expect(peerBlock).toContain("`maestro work accept` is not a command you\nrun");
  expect(peerBlock).not.toContain("maestro team start");
  expect(peerBlock).not.toContain("maestro decide");

  const recipe = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"))
    .text();
  const readme = await Bun.file(join(import.meta.dir, "..", "README.md")).text();
  for (const surface of [recipe, readme]) {
    for (const operation of slpOperations) expect(surface).toContain(operation);
    expect(surface).toContain("~/maestro/SLP.md");
    expect(surface).not.toContain("advisor-<team>");
    expect(surface).not.toContain("observer-<team>");
    expect(surface).not.toContain("maestro-team-sensor");
  }
  // Doctrine review 7: a brief that opens with a capitalised "You are ..." is
  // read by a Claude pane as the slash command /You and dropped silently.
  const shared = pack.match(/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/)?.[1] ?? "";
  for (const surface of [shared, recipe]) {
    expect(surface).toContain("plain lowercase sentence");
    expect(surface).toContain("agent_status=working");
  }
});

function documentedCommands(readme: string): string[][] {
  const verbTour = readme.match(/## Verb tour\n([\s\S]*?)(?=\n## )/)?.[1] ?? "";
  return [...verbTour.matchAll(/`([^`]+)`/g)]
    .map((match) => (match[1] ?? "").replaceAll(/\s+/g, " ").trim())
    .filter((value) => value.startsWith("maestro "))
    .flatMap((value) => {
      const args = value.split(" ").slice(1).filter((arg) => !/^<[^>]+>$/.test(arg));
      const alternatives = args.findIndex((arg) => arg.includes("|"));
      if (alternatives < 0) return [args];
      return (args[alternatives] as string).split("|").map((alternative) =>
        args.map((arg, index) => index === alternatives ? alternative : arg)
      );
    });
}

test("309 [lint] every README verb-tour command resolves through CLI help", async () => {
  // Proves documentation-to-registry resolution, not valid-fixture execution of each command family.
  const readme = await Bun.file(join(import.meta.dir, "..", "README.md")).text();
  const commands = documentedCommands(readme);
  expect(commands).toContainEqual(["dispatch", "list"]);
  expect(commands).toContainEqual(["handback", "show"]);
  expect(commands).not.toContainEqual(["handback", "list"]);

  await withFixture(async (fixture) => {
    for (const command of commands) {
      const args = command[0]?.startsWith("-") || command[0] === "help"
        ? command
        : ["help", ...command];
      const result = await runCli(fixture, args);
      expect({ command: command.join(" "), exitCode: result.exitCode }).toEqual({
        command: command.join(" "),
        exitCode: 0,
      });
    }
  });
  // 17 documented commands, one CLI process each, serially: 1.38s on an idle
  // machine (junit reporter, three runs, 1.32 to 1.39). The default 5000ms
  // tolerates a 3.6x slowdown and nothing more, and it ran out once during a
  // release gate while several agents shared the machine. 30s keeps the test
  // honest about a genuine hang while surviving the contention this repository
  // actually works under.
}, 30_000);

test("310 [lint] supervised-team site guidance matches the registered lifecycle surface", async () => {
  const docsRoot = join(import.meta.dir, "..", "site", "src", "content", "docs");
  const setup = await Bun.file(join(docsRoot, "getting-started", "slp-setup.md")).text();
  const guide = await Bun.file(join(docsRoot, "guides", "supervised-teams.md")).text();
  const roles = await Bun.file(join(docsRoot, "concepts", "roles.md")).text();
  const lanes = await Bun.file(join(docsRoot, "concepts", "lanes.md")).text();
  const readOnlyMode = await Bun.file(join(docsRoot, "guides", "read-only-mode.md")).text();
  const scenarios = await Bun.file(join(docsRoot, "guides", "slp-scenarios.md")).text();
  const reference = await Bun.file(join(docsRoot, "reference", "cli.md")).text();
  const combined = [setup, guide, roles, lanes, readOnlyMode, scenarios, reference].join("\n");

  for (const command of slpOperations) expect(combined).toContain(command);
  expect(guide).toContain("there is no Observer, Advisor, scheduler, health or reconcile");
  expect(guide).toContain("## Runtime pane");
  expect(guide).toContain("It is not an agent");
  expect(guide).toContain("deleted at stop.");
  expect(setup).toContain("~/maestro/SLP.md");
  expect(setup).toContain("<project>/.maestro/SLP.md");
  expect(scenarios).not.toContain("herdr workspace create --cwd ~/Code/rewrite");
  for (const documented of ["### `term`", "### `memory`", "--local", "HUB_UNAVAILABLE", "`import <dir> [--dry-run]`"]) {
    expect(reference).toContain(documented);
  }
  expect(readOnlyMode).toContain("--local");

  await withFixture(async (fixture) => {
    for (const command of [
      ["team"],
      ["team", "start"],
      ["team", "stop"],
      ["status"],
      ["work", "add"],
      ["work", "take"],
      ["work", "note"],
      ["work", "return"],
      ["work", "accept"],
      ["decide"],
    ]) {
      const result = await runCli(fixture, ["help", ...command]);
      expect({ command: command.join(" "), exitCode: result.exitCode }).toEqual({
        command: command.join(" "),
        exitCode: 0,
      });
    }
  });
}, 30_000);

test("311 [lint] SLP v2 is the only registered team architecture and states its cooperative boundary", async () => {
  const root = join(import.meta.dir, "..");
  const pluginNames = builtInPlugins.map((plugin) => plugin.name);
  expect(pluginNames.filter((name) => name === "slp-v2")).toEqual(["slp-v2"]);
  for (const retired of ["team", "team-runtime", "team-observer", "team-sensor", "team-advisor"]) {
    expect(pluginNames).not.toContain(retired);
    expect(await Bun.file(join(root, "src", "plugins", `${retired}.ts`)).exists()).toBe(false);
  }

  const surfaces = await Promise.all([
    Bun.file(join(root, "src", "plugins", "resources", "SLP.md")).text(),
    Bun.file(join(root, "src", "plugins", "recipes", "slp.md")).text(),
    Bun.file(join(root, "README.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "guides", "supervised-teams.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "concepts", "roles.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "getting-started", "slp-setup.md")).text(),
  ]);
  for (const surface of surfaces) {
    expect(surface).toContain("cooperative-agent protocol");
    expect(surface).toContain("not a shell security sandbox");
    expect(surface).toContain("direct Herdr");
  }
});

test("576 [lint] non-SLP procedures share one risk-based workflow without mandatory test generation", async () => {
  // Proves the shipped instructions and scaffold, not an agent's compliance.
  const root = join(import.meta.dir, "..", "src", "plugins");
  const workflow = await Bun.file(join(root, "resources", "WORKFLOW.md")).text();
  for (const heading of ["## Tiers", "## Authorization boundaries", "## Decisions and readiness", "## Testing discipline", "## Recovery and verification", "## Completion and delivery"]) {
    expect(sectionOf(workflow, heading).trim().length).toBeGreaterThan(0);
  }
  expect(sectionOf(workflow, "## Tiers")).toContain("Read-only reconnaissance comes before tier selection");
  expect(sectionOf(workflow, "## Tiers")).toContain("A context reset, session change, or failed attempt alone is not a Full trigger");
  const testing = sectionOf(workflow, "## Testing discipline");
  expect(testing).toContain("Verification is required; new tests are not");
  expect(testing).toContain("Which plausible wrong implementation would this catch");
  expect(testing).toContain("Reuse or extend existing tests first");
  expect(testing).toContain("Stop when acceptance and in-scope risks have sufficient evidence");
  const authority = sectionOf(workflow, "## Authorization boundaries");
  expect(authority).toContain("original user instruction");
  expect(authority).toContain("host permissions");
  expect(authority).toContain("push, merge, release, deploy");
  const recovery = sectionOf(workflow, "## Recovery and verification");
  expect(recovery).toContain("not proof of a design flaw");
  expect(recovery).toContain("equivalent measurement");
  expect(sectionOf(workflow, "## Completion and delivery")).toContain("awaiting delivery approval");

  for (const path of [
    ...["bundle", "design", "work", "verify"].map((name) => `skills/maestro-${name}/SKILL.md`),
    "skills/maestro-design/references/grilling.md",
    "skills/maestro-design/references/domain-modeling.md",
    "skills/maestro-work/references/tdd-antipatterns.md",
    ...["design", "bug-fix", "ship"].map((name) => `recipes/${name}.md`),
  ]) {
    const text = await Bun.file(join(root, path)).text();
    expect({ path, canonical: text.includes("~/maestro/WORKFLOW.md") }).toEqual({ path, canonical: true });
    expect(text).not.toMatch(/before any recon|No tests beyond the SPEC|A never-red test proves nothing|Every settled fork gets a decision|Authorization does not travel|every behavior in scope has a\s+red test|More than\s+two open forks|sessions, a Full trigger/i);
  }

  await withFixture(async (fixture) => {
    await scaffoldRoom(fixture.home);
    expect(await Bun.file(join(fixture.home, "maestro", "WORKFLOW.md")).text()).toBe(workflow);
    const opened = await runCli(fixture, ["bundle", "open", "tier-lint"]);
    expect(opened.exitCode).toBe(0);
    const spec = await Bun.file(join(fixture.repo, ".maestro", "bundle", "tier-lint", "SPEC.md")).text();
    expect(spec).toContain("~/maestro/WORKFLOW.md#testing-discipline");
    expect(spec).not.toContain("Full tier only");
    expect(sectionOf(spec, "## Decisions")).toContain("maestro bundle show tier-lint");
    expect(sectionOf(spec, "## Anti-goals")).toContain("matching VERIFY.md check");
  });
});

test("639 [lint] WORKFLOW.md states the two-model boundary and dispatch/handback help names its scope (Hub d87)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const workflow = await Bun.file(join(root, "resources", "WORKFLOW.md")).text();
  const boundary = workflow.match(/## Two coordination models\n\n([\s\S]*?)\n\n## /)?.[1] ?? "";
  expect(boundary.split("\n\n")).toHaveLength(1);
  expect(boundary).toContain("nine SLP v2 operations");
  for (const classic of ["work start", "decision draft", "ready", "dispatch", "handback", "councils"]) {
    expect(boundary).toContain(classic);
  }
  expect(boundary).toMatch(/not legacy/);

  await withFixture(async (fixture) => {
    for (const verb of ["dispatch", "handback"]) {
      const help = await runCli(fixture, [verb, "--help"]);
      expect(help.exitCode).toBe(0);
      const opening = help.stdout.split("\n")[0] ?? "";
      expect(opening).toMatch(new RegExp(`^${verb} {2,}.*lane contracts outside a running SLP team`, "i"));
    }
  });
});

test("645 [lint] memory help and WORKFLOW.md state the any-cwd rule; uninstall help names room forget (UX F help findings, F10)", async () => {
  const workflow = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "resources", "WORKFLOW.md")).text();
  const memory = workflow.match(/## Memory\n\n([\s\S]*?)\n\n## /)?.[1] ?? "";
  expect(memory).toContain("from any cwd");
  expect(memory).not.toContain("runs from `~/maestro`");
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help", "memory"]);
    expect(help.stdout.split("\n")[0]).toMatch(/^memory {2,}.*from any cwd/);
    expect(help.stdout).not.toContain("retract from the Hub");
    for (const args of [["help", "uninstall"], ["uninstall", "--help"]]) {
      const uninstall = await runCli(fixture, args);
      expect(uninstall.stdout).toContain("maestro room forget <path>");
    }
  });
});

const skillsRoot = join(import.meta.dir, "..", "src", "plugins", "skills");

function sectionOf(text: string, heading: string): string {
  const start = text.indexOf(heading);
  expect({ heading, found: start >= 0 }).toEqual({ heading, found: true });
  const rest = text.slice(start + heading.length);
  const next = rest.search(/\n##+ /);
  return next >= 0 ? rest.slice(0, next) : rest;
}

test("648 [lint] wayfinder: fog notes carry evidence and are cleared by note, an example is one instance, the owner sets the pace (doctrine review 1-4)", async () => {
  const wayfinder = await Bun.file(join(skillsRoot, "maestro-design", "references", "wayfinder.md")).text();
  const fog = sectionOf(wayfinder, "## Fog of war");
  expect(fog).toContain("unverified");
  expect(fog).toMatch(/re-check/);
  expect(wayfinder).not.toContain("drop the fog note");
  expect(fog).toContain("fog cleared by <id>");
  const chart = sectionOf(wayfinder, "### Chart the map");
  const stepOne = chart.split(/\n2\. /)[0] ?? "";
  expect(stepOne).toContain("one instance of it");
  expect(stepOne).toContain("reference tool");
  const invocation = sectionOf(wayfinder, "## Invocation");
  expect(invocation).toContain("Unattended, resolve at most one ticket per session");
  expect(invocation).toContain("With the owner present");
  expect(wayfinder).not.toContain("Never resolve more than one ticket per session");
});

test("649 [lint] grilling: a fork answer is a decision to record, never an implementation order (doctrine review 5)", async () => {
  const grilling = await Bun.file(join(skillsRoot, "maestro-design", "references", "grilling.md")).text();
  const record = grilling.slice(grilling.indexOf("- Record durable decisions"));
  const bullet = record.split("\n\n")[0] ?? "";
  expect(bullet).toContain("never an implementation order");
  expect(bullet).toContain("explicit request");
});

test("650 [lint] the design exit and the bundle tier rule open the bundle in the store whose checkout will change (doctrine review 6)", async () => {
  const design = await Bun.file(join(skillsRoot, "maestro-design", "SKILL.md")).text();
  const bundle = await Bun.file(join(skillsRoot, "maestro-bundle", "SKILL.md")).text();
  const exit = sectionOf(design, "## Readiness gate and exit");
  const full = exit.slice(exit.indexOf("- Full:")).split("\n\n")[0] ?? "";
  const tierRule = sectionOf(bundle, "## Tier rule");
  for (const text of [full, tierRule]) {
    expect(text).toContain("store whose checkout will change");
    expect(text).toContain("hub:<id>");
    expect(text).toContain("Hub map");
  }
});

test("651 [lint] maestro-council: tenth shipped skill carries the Lead-only guard and the d94 unanimity sentence; design and work point at it (w625, hub d92-d95)", async () => {
  expect(skillNames).toContain("maestro-council");
  const council = await Bun.file(join(skillsRoot, "maestro-council", "SKILL.md")).text();
  expect(council).toMatch(/^---\nname: maestro-council\n/);
  expect(council).toContain("<!-- maestro-skill-version: dev -->");
  for (const reference of ["brief.md", "report-format.md"]) {
    expect(await Bun.file(join(skillsRoot, "maestro-council", "references", reference)).exists()).toBe(true);
    expect(council).toContain(`references/${reference}`);
  }

  const guard = sectionOf(council, "## Lead-only guard");
  expect(guard).toContain("Lead of a running team");
  expect(guard).toContain("plain session outside a team");
  expect(guard).toMatch(/A seat never opens a\s+council/);
  expect(guard).toContain("No Observer seat exists (Hub d98)");

  // Hub d94: unanimity opens one premise verifier, never a skip.
  expect(council).toContain(
    "Unanimity is not a skip: above lens, when every valid seat agrees, open exactly one Verifier whose single mandate is to name the shared premise in the brief that drives the common conclusion and test it.",
  );
  expect(council).toContain("COMPROMISED");
  expect(council).toContain("never form an ensemble vote");
  expect(council).toContain("CONCEDE | MAINTAIN | NARROW | REVERSE");
  expect(council).toContain("CLEAR | REVISE | STOP");
  expect(council).toContain("maestro decision draft");
  expect(council).not.toContain("model-routing.md");

  const run = sectionOf(council, "## Run");
  expect(run).toContain("graph run council");
  expect(run).toContain("work add");
  expect(run).toContain("--to peer-<seat>");
  expect(run).toContain("subagent_type: maestro-<seat>");
  expect(run).toContain("spawn_agent");

  const design = await Bun.file(join(skillsRoot, "maestro-design", "SKILL.md")).text();
  const designCouncil = sectionOf(design, "## Council");
  expect(designCouncil).toContain("`maestro-council`");
  expect(designCouncil.trim().split("\n\n")).toHaveLength(1);
  expect(design).not.toContain("eight axes");

  const work = await Bun.file(join(skillsRoot, "maestro-work", "SKILL.md")).text();
  expect(work).toContain("`maestro-council`");
  expect(work).toContain("COUNCIL_REQUEST");
});

test("docs-contract: pack v3 markers, no Observer, --peer-profile and the room d128 model flags named, no --lead-profile, OWNER.md template names the three seat profiles (red 11, item 9)", async () => {
  const root = join(import.meta.dir, "..");
  const pack = await Bun.file(join(root, "src", "plugins", "resources", "SLP.md")).text();
  expect(pack).toContain("<!-- slp:version=3 -->");
  for (const seat of ["team-supervisor", "lead", "peer"]) {
    expect(pack).toContain(`<!-- slp:profile:${seat}=${seat} -->`);
    expect(await Bun.file(join(root, "src", "plugins", "resources", "profiles", `${seat}.md`)).exists()).toBe(true);
  }
  expect(pack).not.toContain("slp:model:");
  expect(pack).not.toContain("slp:role:observer");
  expect(pack).not.toContain("## Observer");
  expect(pack).toContain("--blocked");
  // herdr-adapter (Hub d96, d97): the runtime pane is the second attention
  // layer and the shared section names the lines a seat may receive.
  expect(pack).not.toMatch(/no seat or process watches panes for stalls/);
  expect(pack).toContain("[from runtime][<work-id>]");
  expect(pack).toContain("[attention] <seat> idle");
  expect(pack).not.toContain("slp:watch:begin");
  expect(pack).not.toContain("Watch Pane");

  const recipe = await Bun.file(join(root, "src", "plugins", "recipes", "slp.md")).text();
  const cli = await Bun.file(join(root, "site", "src", "content", "docs", "reference", "cli.md")).text();
  const readme = await Bun.file(join(root, "README.md")).text();
  for (const surface of [recipe, cli]) {
    expect(surface).toContain("--peer-profile");
    expect(surface).not.toContain("--lead-profile");
    expect(surface).not.toContain("--observer-model");
    expect(surface).not.toContain("sentinel");
  }
  expect(cli).not.toContain("--stall repeat");
  // room d128 (w728): a seat's model is a launch-time choice. The Hub
  // Supervisor section of the pack, the README and the CLI reference name
  // the three team start flags and say they change the model only, never
  // the mandate or the pinned digest; the retired-flag refusal is gone.
  const hub = /<!-- slp:role:hub-supervisor:begin -->([\s\S]*?)<!-- slp:role:hub-supervisor:end -->/.exec(pack)?.[1] ?? "";
  for (const surface of [hub, readme, cli]) {
    for (const flag of ["--lead-model", "--peer-model", "--supervisor-model"]) expect(surface).toContain(flag);
    expect(surface).not.toContain("RETIRED_FLAG");
  }
  expect(hub).toContain("never its mandate or the pinned profile digest");
  expect(cli).toContain('maestro work add "<objective>" --to <peer-name> --model <model>');

  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    const owner = await Bun.file(join(room, "OWNER.md")).text();
    for (const seat of ["`team-supervisor`", "`lead`", "`peer`"]) expect(owner).toContain(seat);
    expect(owner).toContain("profiles/<name>.md");
    expect(owner).not.toContain("| rung |");
  });
});

// seat-config-dirs R8 (d847): a Peer reaches the Lead and other Peers only
// through recorded notes and returns; the shipped shared contract and peer
// mandate no longer tell it to prompt panes by hand, while the Lead and the
// Team Supervisor keep the hand-typed ask.
test("seat-dirs-doctrine: the shipped SLP.md shared contract and peer.md drop the Peer's direct hand-prompt wording; the Lead and Team Supervisor keep theirs (R8, d847)", async () => {
  const root = join(import.meta.dir, "..");
  const pack = await Bun.file(join(root, "src", "plugins", "resources", "SLP.md")).text();
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const shared = unwrap(/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "");
  expect(shared).not.toContain("Communicate directly along the team topology");
  expect(shared).not.toContain("When you prompt a pane by hand");
  expect(shared).not.toContain("Hand-typed asks are allowed");
  expect(shared).toContain("A Peer reaches the Lead and other Peers only through recorded work notes and returns");
  expect(shared).toContain("The Lead and the Team Supervisor may add a hand-typed ask");
  expect(shared).toContain("herdr agent prompt");

  const profiles = join(root, "src", "plugins", "resources", "profiles");
  // room d134: the same doctrine, in words that name nobody to reach.
  const peer = unwrap(await Bun.file(join(profiles, "peer.md")).text());
  expect(peer).not.toContain("Communicate directly");
  expect(peer).not.toContain("herdr agent prompt");
  expect(peer).toContain("record `maestro work note <work-id> \"<the premise, the evidence, and what the request would become>\" --blocked`");
  expect(unwrap(block(pack, "shared:peer"))).toContain("There is no one to hand it to and no dialog to ask a question through");
  expect(unwrap(await Bun.file(join(profiles, "lead.md")).text())).toContain("Communicate directly with the Team Supervisor and every Peer");
  expect(unwrap(await Bun.file(join(profiles, "team-supervisor.md")).text())).toContain("Communicate directly with the Hub Supervisor, the Lead, and every Peer");

  const recipe = unwrap(await Bun.file(join(root, "src", "plugins", "recipes", "slp.md")).text());
  expect(recipe).not.toContain("The Team Supervisor, Lead, and Peers may talk directly");
  expect(recipe).toContain("recorded work notes and returns");
});

// room d118: the Lead self-works its items by default and opens a Peer the way
// a harness opens a sub-agent; the Lead-must-not-implement rule is replaced by
// "may implement its own items and any item not already assigned to a Peer".
// room d117: a generation may run lead-only, where the Hub Supervisor is the
// Lead's reviewer; the supervised shape stays the default.
test("slp-doctrine: the shipped Lead mandate makes self-work the default and Peers sub-agent-shaped, and the pack carries the lead-only shape in the shared contract and the Hub Supervisor section (room d117, room d118)", async () => {
  const root = join(import.meta.dir, "..");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const lead = unwrap(await Bun.file(join(root, "src", "plugins", "resources", "profiles", "lead.md")).text());

  // d118: implementing is the Lead's default path, not the Peer brief.
  expect(lead).toContain("Working your own items is the default path");
  expect(lead).toContain("You may implement your own items and any item you have not already assigned to a Peer");
  expect(lead).toContain("Open a Peer the way a harness opens a sub-agent, never as the default path for every item");
  expect(lead).not.toContain("never implement a Peer's item yourself");
  expect(lead).not.toContain("Brief every Peer");
  // What d118 does not touch: an item already delegated, context hygiene,
  // review of every return, and acceptance that is never the Lead's own.
  expect(lead).toContain("An item that is already a Peer's stays the Peer's");
  expect(lead).toContain("read returns through `maestro status <work-id>`, not pane transcripts");
  expect(lead).toContain("Review every Peer return before accepting it with `maestro work accept`");
  expect(lead).toContain("you never accept your own");

  const pack = await Bun.file(join(root, "src", "plugins", "resources", "SLP.md")).text();
  const shared = unwrap(/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "");
  expect(shared).toContain("The supervised shape is the default");
  expect(shared).toContain("lead-only shape (`maestro team start --lead-only`): one Lead plus the runtime pane and no Team Supervisor");
  expect(shared).toContain("the Lead's reviewer is the Hub Supervisor");
  expect(shared).toContain("the Lead's `--blocked` note escalates to the Hub");
  expect(shared).toContain("a Peer's reviewer is still its Lead in either shape");

  const hub = unwrap(/<!-- slp:role:hub-supervisor:begin -->([\s\S]*?)<!-- slp:role:hub-supervisor:end -->/.exec(pack)?.[1] ?? "");
  expect(hub).toContain("you are the Lead's reviewer");
  expect(hub).toContain("accept it or grant `--rework` on it");
  expect(hub).toContain("That reach covers a lead-only team's Lead only");
  expect(hub).toContain("in a supervised team you still go through the Team Supervisor");

  // The shape is doctrine text only: the locked surface and pack version stand.
  expect(pack).toContain("<!-- slp:version=3 -->");
  const publicSurface = (
    pack.match(/The public SLP surface is exactly:\n\n```text\n([\s\S]*?)\n```/)?.[1] ?? ""
  ).split("\n");
  expect(publicSurface).toEqual([...slpOperations]);
});

// room d122-d126: the collaboration and autonomy layer is doctrine text plus
// three small mechanisms (work note --owner, maestro owner, decide
// --provisional / decision confirm). The pack version, marker lines and the
// nine-operation surface stay byte-unchanged: `maestro owner` and
// `maestro decision confirm` are Hub-room verbs like `decision`, not SLP
// operations.
test("slp-doctrine: the shipped pack carries the --blocked ladder, the owner walk-in, presence and provisional rulings, and every profile knows its part (room d122-d126)", async () => {
  const root = join(import.meta.dir, "..");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const read = async (...parts: string[]) => Bun.file(join(root, "src", "plugins", "resources", ...parts)).text();
  const pack = await read("SLP.md");
  const shared = unwrap(/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "");
  const hub = unwrap(/<!-- slp:role:hub-supervisor:begin -->([\s\S]*?)<!-- slp:role:hub-supervisor:end -->/.exec(pack)?.[1] ?? "");

  // d122: one seat at a time, every seat filters, the Team Supervisor embodies
  // the owner, the Hub takes its part in a lead-only generation.
  expect(shared).toContain("A `--blocked` fork climbs one seat at a time and every seat filters");
  expect(shared).toContain("a technical question is answered by the seat that owns it (a Lead decides)");
  expect(shared).toContain("The Team Supervisor is the owner's embodiment inside its team");
  expect(shared).toContain("asks the Hub with its own `--blocked` note only when it cannot");
  expect(shared).toContain("The Hub decides what a Hub Supervisor may decide and puts to the owner only what is hers");
  expect(shared).toContain("in a lead-only generation the Hub takes the Team Supervisor's part for its Lead");
  // d123: the walk-in note.
  expect(shared).toContain('record it first as `maestro work note <id> "owner asked: <what>" --owner`');
  expect(shared).toContain("a provenance flag exclusive with `--blocked` and `--rework` that stops nobody, sets no blocked flag");
  expect(shared).toContain("pushes `[from <role>][<work-id> OWNER]` one seat up");
  expect(shared).toContain("Inside the item's objective act at once; outside it, record the `--owner` note and do not widen the item: the Lead reads the push and opens new work");
  expect(shared).toContain("A walk-in never rewrites an objective or an acceptance, and never redefines a seat's mandate");

  // d124, d125 and the two w722 leftovers live in the Hub Supervisor section.
  expect(hub).toContain("`maestro owner here|away`, a Hub-room verb like `decision`, not an SLP operation");
  expect(hub).toContain("no hook and no other seat infers it, and a `[from <role>]` push line never counts as the owner");
  expect(hub).toContain("`maestro owner` prints it, and the Hub prompt line and `maestro status` carry `owner: here|away`");
  expect(hub).toContain("Presence changes only who answers an owner-scope `--blocked` note that has reached you");
  expect(hub).toContain("`here`, you wait for the owner; `away`, you resolve it at once through advisor");
  expect(hub).toContain('`maestro decide "<choice>" --why "<reason>" --work <id> --provisional`');
  expect(hub).toContain("`maestro status` lists every provisional ruling until she confirms it with `maestro decision confirm <id>` or supersedes it with a decision that `--replaces` it");
  expect(hub).toContain("`away` never grants commit, push, tag, release, install mid-generation, or any destructive change");
  expect(hub).toContain("You also run the normal stop of a lead-only team, `maestro team stop <team-id> --reason \"<closing report>\"`");
  expect(hub).toContain("a supervised team you stop with `--emergency` only");

  // Every seat profile knows the walk-in and its place on the ladder.
  const lead = unwrap(await read("profiles", "lead.md"));
  expect(lead).toContain('`maestro work note <id> "owner asked: <what>" --owner`');
  expect(lead).toContain("outside it open new work");
  expect(lead).toContain("never widen the item");
  expect(lead).toContain("a technical fork you own you decide");
  const supervisor = unwrap(await read("profiles", "team-supervisor.md"));
  expect(supervisor).toContain("You are the owner's embodiment inside the team");
  expect(supervisor).toContain("by your own judgment or through advisor");
  expect(supervisor).toContain("only what you cannot resolve");
  expect(supervisor).toContain('`maestro work note <id> "owner asked: <what>" --owner`');
  // room d134: the walk-in reaches the Peer through the pack's Peer block, and
  // it names no seat that opens the follow-up work.
  const peerBlock = unwrap(block(pack, "shared:peer"));
  expect(peerBlock).toContain('`maestro work note <work-id> "owner asked: <what>" --owner`');
  expect(peerBlock).toContain("new work will come for it");
  expect(peerBlock).not.toContain("the Lead opens the work");

  // Byte-unchanged: version, marker lines, and the locked surface.
  expect(pack).toContain("<!-- slp:version=3 -->");
  expect(pack).toContain("<!-- slp:profile:team-supervisor=team-supervisor -->\n<!-- slp:profile:lead=lead -->\n<!-- slp:profile:peer=peer -->");
  const publicSurface = (
    pack.match(/The public SLP surface is exactly:\n\n```text\n([\s\S]*?)\n```/)?.[1] ?? ""
  ).split("\n");
  expect(publicSurface).toEqual([...slpOperations]);
  expect(publicSurface).not.toContain("maestro owner");
});

// room lessons l16, l21, l22, l23, l25: the room's own evidence rule. The
// intake line already sends a question about the room to IDENTITY.md, so the
// corrections land there rather than in the workflow every repository reads.
test("653 [lint] the room's IDENTITY.md carries the evidence rule for documents, instruments and tool refusals (l16, l21, l22, l23, l25)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    const identity = (await Bun.file(join(room, "IDENTITY.md")).text()).replaceAll(/\s+/g, " ");
    expect(identity).toContain("A tool verdict is an observation, and a document is weaker still");
    expect(identity).toContain("Read a constant through the function that consumes it");
    expect(identity).toContain("measure an environment precondition in the environment");
    expect(identity).toContain("The link to verify is the one the remedy hangs on");
    expect(identity).toContain("Before an instrument asserts a negative it must first show a positive on this machine");
    expect(identity).toContain("`<symptom> observed, cause not measured`");
    expect(identity).toContain("it must read differently from a true zero");
    expect(identity).toContain("State the question the instrument answers before quoting its answer");
    expect(identity).toContain("A tool's refusal is evidence about the tool");
    expect(identity).toContain("measure that claim yourself before recording anything that rests on it");
  });
});

// room lessons l17, l18: the room's snapshot watcher wedged silently and was
// still described as live cover, and its input set emptied on the first
// commit - the success it was waiting for killed it.
test("654 [lint] the room's IDENTITY.md tells a monitor to prove its own liveness and never define itself over a set that empties on success (l17, l18)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    const identity = (await Bun.file(join(room, "IDENTITY.md")).text()).replaceAll(/\s+/g, " ");
    expect(identity).toContain("emits a liveness line every cycle");
    expect(identity).toContain("its silence reads as failure rather than as quiet");
    expect(identity).toContain("its input set never empties when the work goes right");
    expect(identity).toContain("Re-measure the monitor before citing it");
    expect(identity).toContain("a watcher that fails only on new arrivals passes every spot-check");
  });
});

// room lesson l12: two honest gate reports on the identical tree disagreed
// twenty minutes apart, and the only difference was what else was running.
test("655 [lint] the room's IDENTITY.md makes a gate result name the machine's load and re-run a loaded-machine timeout (l12)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    const identity = (await Bun.file(join(room, "IDENTITY.md")).text()).replaceAll(/\s+/g, " ");
    expect(identity).toContain("A gate result names the machine's concurrent load");
    expect(identity).toContain("A red that is only a timeout on a loaded machine is re-run on a quiet one");
    expect(identity).toContain("one fact about the box, not two facts about the tree");
  });
});

// room lesson l19: in one hour a mistyped -only-testing target reported GREEN
// WITH NOTHING RUN, an appended `; echo EXIT=0` turned a red run into exit 0,
// and a stale gitignored generated project invited a false accusation.
test("656 [lint] WORKFLOW.md believes a run on what executed, not on an exit code (l19)", async () => {
  const workflow = (await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "WORKFLOW.md"),
  ).text()).replaceAll(/\s+/g, " ");
  expect(workflow).toContain("Believe a run on the count of what actually executed");
  expect(workflow).toContain("never on an exit code or a summary line");
  expect(workflow).toContain("a pass with a suspiciously small count as the finding");
  expect(workflow).toContain("whose last command is not the thing under test");
  expect(workflow).toContain("check the shared generated and ignored inputs first");
  expect(workflow).toContain("Distrust hardest the explanation that removes your reason to look further");
});

// room lesson l24: two safety clauses this room wrote converted into
// permissions when their conditions lapsed with nobody deciding anything.
test("657 [lint] WORKFLOW.md words safety clauses in the affirmative polarity (l24)", async () => {
  const workflow = (await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "WORKFLOW.md"),
  ).text()).replaceAll(/\s+/g, " ");
  expect(workflow).toContain("Word a safety clause in the affirmative");
  expect(workflow).toContain('"X happens only when <a named party does Y>"');
  expect(workflow).toContain("A permission must be created by an act");
  expect(workflow).toContain("never by a state ceasing");
  expect(workflow).toContain("the condition's expiry coincides with the hazard's end");
});

// room lesson l13: a shadow edit to profiles/lead.md while two generations
// were RUNNING blocked every Lead peer dispatch in both teams, and nothing the
// room reads said team start pins all three seat profiles.
test("658 [lint] the Hub section and the SLP recipe say team start pins every launched seat profile, so shadow edits wait for a generation boundary (l13)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const pack = await Bun.file(join(root, "resources", "SLP.md")).text();
  const hub = unwrap(
    /<!-- slp:role:hub-supervisor:begin -->([\s\S]*?)<!-- slp:role:hub-supervisor:end -->/
      .exec(pack)?.[1] ?? "",
  );
  expect(hub).toContain("pins the bytes of every seat profile the generation launches");
  expect(hub).toContain("whether or not that generation ever opens a Peer");
  expect(hub).toContain("`SLP_SNAPSHOT_CHANGED`");
  expect(hub).toContain("a shadow edit belongs at a generation boundary");
  const recipe = unwrap(await Bun.file(join(root, "recipes", "slp.md")).text());
  expect(recipe).toContain("the bytes of every seat profile it launches");
  expect(recipe).toContain("whether or not it ever opens a Peer");
  expect(recipe).not.toContain("every profile it referenced");
});

// room lesson l14: the room told a team the runtime was ten commits behind
// when the true figure was one, and the team had refused maestro update partly
// on the size of the ten. The single commit was w700, the one that changes how
// profile shadows resolve.
test("659 [lint] the shared contract makes a correction that lowers a risk figure carry the conclusion it does not change (l14)", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("A correction that makes a risk figure smaller travels with the conclusion it does not change");
  expect(shared).toContain("say whether the ruling moves");
  expect(shared).toContain("give the reason that survives the new number");
  expect(shared).toContain("will reason its way to the action that was refused");
});

// room lesson l20: five contracts named "the suite green under DEVELOPER_DIR"
// as their acceptance clause; a later ruling made that mechanism unavailable to
// every seat, and because acceptance never changes, all five became
// permanently unacceptable however good the work was.
test("660 [lint] the shared contract requires an acceptance clause the receiving seat can discharge (l20)", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("an acceptance clause must be dischargeable by the seat that receives it");
  expect(shared).toContain("a mechanism that seat does not control");
  expect(shared).toContain("state in advance the exact words for reporting it unproven");
  expect(shared).toContain("rather than a contract nobody can accept");
});

// room lessons l15 and l26: a stop that had to land before the peer's next
// tool call went out as plain work notes, which wake nobody; and l8's
// described remedy for zsh command substitution in a brief recurred, so the
// remedy is now a shipped shell function and the improver is told to prefer a
// mechanism over a sentence.
test("662 [lint] the pack, the recipe and the improver name the waking channel and the file-based brief (l15, l26)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const pack = await Bun.file(join(root, "resources", "SLP.md")).text();
  const shared = unwrap(/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "");
  // w740 (d899): decide joined the four, so the count in this sentence moved
  // with it; the rule it states - a waking operation, never a plain note - is
  // what l15 pins and is unchanged.
  expect(shared).toContain("An instruction whose value expires before the recipient's next action travels on one of those five waking operations, never on a plain note");
  expect(shared).toContain("a pane inside a tool call is not a slow reader but no reader at all");
  expect(shared).toContain("work already running cannot be stopped from outside its own pane");
  expect(shared).toContain("`mbrief <agent> <file>` in the room's shellrc, never as an inline double-quoted argument");
  expect(shared).toContain("zsh performs command substitution inside double quotes");
  const recipe = unwrap(await Bun.file(join(root, "recipes", "slp.md")).text());
  expect(recipe).toContain("`mbrief <agent> <file>` from the room's shellrc");
  expect(recipe).toContain("never on a plain note");
  const skill = unwrap(await Bun.file(join(root, "skills", "maestro-improve", "SKILL.md")).text());
  expect(skill).toContain("A lesson whose hazard is mechanical is not answered by a sentence");
  expect(skill).toContain("the return names which mechanism now makes the recurrence impossible");
});

// room d129 point 4: the two capture points are recorded, and the shared
// contract is where the seat and its reviewer learn what the mark asks for.
test("668 [lint] the shared contract names the lesson-candidate mark and what the return and the reviewer owe it (room d129)", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("mark that item as carrying a lesson candidate");
  expect(shared).toContain("never a reading of anyone's text");
  expect(shared).toContain("states either the lesson it filed (`maestro lesson file`, with its id) or why none applies");
  expect(shared).toContain("accepts only after the return states one of the two");
  expect(shared).toContain("`maestro lesson file` writes the store of the project it runs in");
  // The nine-operation surface is unchanged: no new flag rides work return.
  expect(shared).not.toContain("work return <id> --lesson");
});

// Hub finding F7 (w734): four times since g6 a Hub read a seat pane and saw a
// line such as "return w730" or "[w727 RETURNED]" sitting in the input box
// before the store held any such record - the harness's own prompt suggestion,
// not a push. The contract says beside the wake-up sentence that a pane is not
// a record.
test("669 [lint] the shared contract says pane text is not a push and a state exists only in the store (Hub finding F7)", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("text seen in a pane's input box or transcript is not a push and not a record");
  expect(shared).toContain("a work state exists only when `maestro status <work-id>` shows it");
  expect(shared).toContain("checks the store before acting on what it saw");
  // It belongs beside the sentence it qualifies: the store is the truth and
  // the pushed line is only the wake-up.
  const wakeUp = shared.indexOf("the store stays the truth and that line is only the wake-up");
  const phantom = shared.indexOf("text seen in a pane's input box or transcript is not a push");
  expect(wakeUp).toBeGreaterThan(-1);
  expect(phantom).toBeGreaterThan(wakeUp);
  expect(phantom - wakeUp).toBeLessThan(200);
});

// room d135: one HR protocol for owned work. A harness sub-agent leaves no
// store row, cannot return and cannot be accepted, so owned work runs in a
// Herdr pane and a sub-agent is read-only scouting - including a
// maestro-graph agent node.
test("673 [lint] the shared contract, WORKFLOW.md and maestro-graph say owned work runs in a pane and a sub-agent only scouts (room d135)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const pack = await Bun.file(join(root, "resources", "SLP.md")).text();
  const shared = unwrap(
    /<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "",
  );
  expect(shared).toContain(
    "Work that has an owner, writes files, returns or is accepted runs in a Herdr pane through the SLP operations and leaves a store row",
  );
  expect(shared).toContain(
    "a harness sub-agent (Agent, Task or Workflow) is used only for read-only scouting that returns text and writes nothing",
  );
  // It belongs beside the paragraph that already tells a Peer which channels
  // it has, where `herdr` is named.
  const herdr = shared.indexOf("its seat denies `herdr`");
  const protocol = shared.indexOf("Work that has an owner, writes files, returns or is accepted");
  expect(herdr).toBeGreaterThan(-1);
  expect(protocol).toBeGreaterThan(herdr);
  expect(protocol - herdr).toBeLessThan(200);
  const workflow = unwrap(await Bun.file(join(root, "resources", "WORKFLOW.md")).text());
  expect(workflow).toContain(
    "Work that has an owner, writes files, returns or is accepted runs in a Herdr pane through the SLP operations and leaves a store row",
  );
  expect(workflow).toContain(
    "give a harness sub-agent (Agent, Task, Workflow, or a maestro-graph agent node) only read-only scouting that returns text and writes nothing",
  );
  const graph = await Bun.file(join(root, "skills", "maestro-graph", "SKILL.md")).text();
  const description = /^description:.*$/m.exec(graph)?.[0] ?? "";
  expect(description).toContain("A sub-agent node is read-only scouting");
  expect(description).toContain("a node that writes files or is accepted is opened as a pane");
  const body = unwrap(graph);
  expect(body).toContain(
    "A sub-agent node is read-only scouting that returns text and writes nothing; a node whose work writes files, returns or is accepted is opened as a pane with `maestro work add --to <peer>` and its result handed back with `graph result` as before.",
  );
  expect(body).toContain("kind agent -> spawn a read-only scouting sub-agent");
});

// room d138: the anti-pattern vocabulary ships as one file beside SLP.md and
// WORKFLOW.md, the seven named patterns in the order the giáo án walked them and the article's parachute after them, each
// with the maestro evidence where one exists and "not yet seen here" where
// none does, so a filed lesson can name the pattern it shows.
test("674 [lint] ANTI_PATTERN.md carries the eight patterns in order and WORKFLOW.md and the improver point at it (room d138)", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins");
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const path = join(root, "resources", "ANTI_PATTERN.md");
  expect(await Bun.file(path).exists()).toBe(true);
  const text = await Bun.file(path).text();
  expect([...text.matchAll(/^## (.+)$/gm)].map((match) => match[1])).toEqual([
    "balloon",
    "brake",
    "pre-solve",
    "weak scout",
    "polling waste",
    "dual ownership",
    "priority by label",
    "parachute",
  ]);
  // Every entry states its evidence or says plainly that there is none yet.
  for (const section of text.split(/^## /m).slice(1)) {
    expect(section).toContain("evidence:");
  }
  // room d134 re-worded the mandate this quotes; the balloon entry has to keep
  // quoting what profiles/peer.md actually says, or its evidence is a fiction.
  expect(unwrap(text)).toContain("work you notice beyond this one is `maestro work note");
  expect(unwrap(await Bun.file(join(root, "resources", "profiles", "peer.md")).text()))
    .toContain("work you notice beyond this one is `maestro work note");
  expect(unwrap(text)).toContain("not yet seen here");
  const workflow = unwrap(await Bun.file(join(root, "resources", "WORKFLOW.md")).text());
  expect(workflow).toContain("ANTI_PATTERN.md");
  expect(workflow).toContain("a lesson names the anti-pattern it shows when one fits");
  const skill = unwrap(await Bun.file(join(root, "skills", "maestro-improve", "SKILL.md")).text());
  expect(skill).toContain("ANTI_PATTERN.md");
});

// w736 F1: the Hub's plain note down into a lead-only team is a new line in
// the protocol, so the Hub section is where it is written and this is the lint
// that keeps it written. The Hub hand-copies from this section; a sentence it
// cannot quote is a sentence it does not have.
test("w736 [lint] the Hub section carries the plain note down into a lead-only team, its push line, and the two flags that stay refused", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const hub = (/<!-- slp:role:hub-supervisor:begin -->([\s\S]*?)<!-- slp:role:hub-supervisor:end -->/
    .exec(pack)?.[1] ?? "").replaceAll(/\s+/g, " ");
  expect(hub).toContain('you may also record a plain `maestro work note <id> "<text>"` on the Lead\'s item');
  expect(hub).toContain("the reviewer answer that is a fact or a pointer and not a ruling");
  expect(hub).toContain("it reaches the Lead's pane as `[from hub-supervisor][<id> NOTE]`");
  expect(hub).toContain("it is the only plain note in SLP that wakes anyone");
  expect(hub).toContain("`--blocked` and `--owner` from the room stay refused");
  expect(hub).toContain("an owner instruction typed into your pane is recorded as `maestro decide --scope owner`");
  expect(hub).toContain("A work id only the room's own work table holds is still the room's own item");
  expect(hub).toContain("an id both spaces hold is refused until you qualify the team's as `<team-id>:<id>`");
  expect(hub).toContain("any other id is resolved as SLP work, so a miss says `SLP work not found from Hub`");
  // The shared contract's rule for seats is untouched: it is the Hub that is
  // the exception, not the plain note.
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("never on a plain note");
});

// w740 (room d151-d155): the question-back seat is written in four places and
// nowhere else - the shared contract every seat reads, the two mandates that
// act on it, and the design skill that holds the designer's obligation. The
// Hub hand-copies the SLP.md sentences into ~/maestro/SLP.md; a sentence it
// cannot quote is a sentence it does not have.
test("w740 [lint] the question-back seat's sentences are in the pack, the two mandates and the design skill", async () => {
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const resources = join(import.meta.dir, "..", "src", "plugins", "resources");
  const flat = async (...parts: string[]) =>
    unwrap(await Bun.file(join(resources, ...parts)).text());

  const pack = await flat("SLP.md");
  // d899: decide is the fifth waking operation, named in the same sentence as
  // the other four so the two cannot drift apart.
  expect(pack).toContain(
    "After `maestro work add`, `maestro work return`, `maestro work accept`, " +
      "`maestro work note --rework`, and `maestro decide --work <id>` commit",
  );
  expect(pack).toContain("a decision wakes the seat holding the item it names, under the subject `DECIDED`");
  expect(pack).toContain("travels on one of those five waking operations, never on a plain note");
  // d898: the signal, its one-per-design rule and its silence outside a team.
  expect(pack).toContain(
    "`maestro bundle open <id> --work <work-id>` is the store's signal that a design phase has begun",
  );
  expect(pack).toContain("once, on the open that creates the bundle and never on a resume");
  expect(pack).toContain("nothing at all outside a running team");
  // d902/d154: who opens it in each shape.
  expect(pack).toContain(
    "a Team Supervisor does so with its own `maestro work add --to peer-questioner`, " +
      "which is the one Peer its `work add` may name",
  );
  expect(pack).toContain(
    "the Hub, which manages no Peer, records `maestro decide --work <work-id>` instead " +
      "and the Lead executes that ruling with the same `work add`",
  );
  // d152/d900: what it produces and what the designer owes it.
  expect(pack).toContain("its questions arrive as its own return");
  expect(pack).toContain(
    "answers each, by note or by changing the SPEC, before `maestro decide` locks the point it doubts",
  );

  const lead = await flat("profiles", "lead.md");
  expect(lead).toContain("`maestro work add --to peer-questioner");
  expect(lead).toContain("That is execution, not a choice of seat");
  expect(lead).toContain("[from <who>][<work-id> DECIDED] <choice>");
  expect(lead).toContain(
    "before `maestro decide` locks a point one of them doubts, answer that question",
  );

  const supervisor = await flat("profiles", "team-supervisor.md");
  expect(supervisor).toContain("`maestro work add` to the Lead or to `peer-questioner`");
  expect(supervisor).toContain("[from lead][<work-id> DESIGN] design bundle <id>");
  expect(supervisor).toContain("It is the one Peer your `work add` may name");
  expect(supervisor).toContain("You do not open it on every item or every return");

  // d904/d155: the profile's frontmatter is the read-only contract, and the
  // model stays a launch-time choice (room d128).
  const questioner = await Bun.file(join(resources, "profiles", "questioner.md")).text();
  expect(questioner).toContain("harness: claude\n");
  expect(questioner).toContain("model: default\n");
  expect(questioner).toContain("disallowed_tools: [Write, Edit, NotebookEdit, AskUserQuestion]\n");
  // The whole file, frontmatter included: the description is rendered into the
  // agent file the seat reads, so it is held to the same words as the body.
  const flatQuestioner = unwrap(questioner);
  expect(flatQuestioner).toContain("You settle nothing and you change no file");
  expect(flatQuestioner).toContain("Ask one to three questions, no more");
  expect(flatQuestioner).toContain("names the evidence that would settle it");
  expect(flatQuestioner).toContain("maestro work return <work-id>");
  // room d134: it reads the Peer block, so its own words name no rank either.
  // tests/peer-pack.test.ts measures the rendered seat; this measures the file.
  for (const forbidden of ["Role: Peer", "the Lead", "Team Supervisor", "SLP", "seat", "generation"]) {
    expect({ forbidden, file: flatQuestioner })
      .toEqual({ forbidden, file: expect.not.stringContaining(forbidden) });
  }

  const design = unwrap(
    await Bun.file(join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-design", "SKILL.md")).text(),
  );
  expect(design).toContain("the answer may be a question-back pass opened on it");
  expect(design).toContain("returns one to three questions, each naming the claim it doubts");
  expect(design).toContain(
    "Before `maestro decide` locks a point one of those questions doubts, answer that question",
  );
  expect(design).toContain("A point locked over an unanswered question is a decision taken without the evidence it asked for");
});

// project lesson l8 (w739, d896): an evidence command with its stderr thrown
// away reported a clean negative it could not have reported anything else from,
// and that silence became a plank under the claim that a Peer had delivered
// nothing. The rule lands in the shared testing discipline and again in the one
// mandate that verifies another seat's claim.
test("l8 [lint] WORKFLOW.md and the Team Supervisor mandate falsify an evidence command before believing its negative", async () => {
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const resources = join(import.meta.dir, "..", "src", "plugins", "resources");
  const flat = async (...parts: string[]) => unwrap(await Bun.file(join(resources, ...parts)).text());

  const workflow = await flat("WORKFLOW.md");
  expect(workflow).toContain("A command run as evidence shows its stderr");
  expect(workflow).toContain(
    "believed only after that same command has produced a positive on a case known to be positive",
  );
  expect(workflow).toContain("Where the claim is that nothing happened, falsify the check first");
  expect(workflow).toContain("An empty result and a broken command are the same text on stdout");

  const supervisor = await flat("profiles", "team-supervisor.md");
  expect(supervisor).toContain("show that command's stderr");
  expect(supervisor).toContain("believe a negative from it only after it has produced a positive on a case you know is positive");
  expect(supervisor).toContain("An empty result and a broken command print the same nothing");
});

// project lesson l5 (w730, w731): eleven doctrine commits passed per-edit gates
// chosen by the file being edited, and the full suite at the end found two reds
// those edits had caused in assertions living elsewhere.
test("l5 [lint] the improver picks its per-edit gate by the output it changes, not by the file", async () => {
  const skill = (await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-improve", "SKILL.md"),
  ).text()).replaceAll(/\s+/g, " ");
  expect(skill).toContain("The replay is necessary and never sufficient");
  expect(skill).toContain("most doctrine output is exercised by no scenario at all");
  expect(skill).toContain(
    "Choose the rest of the per-edit gate by the output the edit changes, not by the file it changed",
  );
  expect(skill).toContain("grep the whole test tree for the string you are changing");
  expect(skill).toContain("run every suite that asserts it");
  // w743 F3: one of the two reds l5 cites is a corpus-wide property lint that
  // quotes no string of the edit's, so a grep alone would still have missed it.
  expect(skill).toContain("it cannot find a property lint that walks a corpus");
  expect(skill).toContain("because such a lint names no string of yours");
  expect(skill).toContain("also run the suite that owns the corpus the edited file belongs to");
});

// project lesson l7 (w732, w733, d880, d889): a SPEC locked an 11-module split
// on a hand tally that was never written down; re-deriving it found a real
// cycle the tally had asserted away and per-module counts off by up to 56 lines.
// The deliverable list is the instrument that was missing a slot.
test("l7 [lint] the SPEC template and the design skill ship the artifact a locked decision rests on", async () => {
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const bundle = unwrap(
    await Bun.file(join(import.meta.dir, "..", "src", "plugins", "bundle.ts")).text(),
  );
  expect(bundle).toContain("## Derived artifacts");
  expect(bundle).toContain(
    "A table, map, graph or measurement any decision above rests on ships in this bundle beside SPEC.md",
  );
  expect(bundle).toContain("what that method over- or under-approximates");
  expect(bundle).toContain("The conclusion is not the evidence");

  const design = unwrap(
    await Bun.file(
      join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-design", "SKILL.md"),
    ).text(),
  );
  expect(design).toContain("ships that artifact in the bundle beside SPEC.md");
  expect(design).toContain("under `## Derived artifacts`");
  expect(design).toContain("what the method over- or under-approximates");
  expect(design).toContain(
    "a VERIFY check that asks a later reviewer to re-derive what the design claimed to have settled is the sign the artifact is missing",
  );
});

// project lesson l9 (w739, d896): the text read printed one line of a 4,000-word
// objective and said nothing about the rest, so the "read the request in full"
// rule was unfollowable by the reader who obeyed it most literally.
test("l9 [lint] the shared contract and the Peer block send a seat to --json for the whole objective", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
  const section = (name: string) =>
    unwrap(new RegExp(`<!-- slp:${name}:begin -->([\\s\\S]*?)<!-- slp:${name}:end -->`).exec(pack)?.[1] ?? "");

  const shared = section("shared");
  expect(shared).toContain("That text read shows one line of the objective and says so when there is more");
  expect(shared).toContain(
    "read your item's objective in full with `maestro status <work-id> --json` before the first edit",
  );
  expect(shared).toContain("an item you have read in part you have not read");

  const peer = section("shared:peer");
  expect(peer).toContain("Its text shows one line of the request and says so when there is more");
  expect(peer).toContain("read the request in full with `maestro status <work-id> --json` before you start");
  // The old claim that the text read prints the item in full is what made the
  // doctrine unfollowable; it does not come back.
  expect(peer).not.toContain("prints one in full");
});

// project lesson l4 (w719, w720, d121): 'room decisions d117 and d118' resolved
// against the project store, which held two real, locked, plausible rows about
// something else. The convention is d121's; the contract is where a seat reads
// it, and the refusal to resolve a bare id is what makes it actionable.
test("l4 [lint] the shared contract store-qualifies a decision id crossing the Hub-to-team boundary", async () => {
  const pack = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"),
  ).text();
  const shared = (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
  expect(shared).toContain("number decisions from `d1` independently");
  expect(shared).toContain(
    "a decision id that crosses the Hub-to-team boundary is written `room dN` or `project dN`",
  );
  expect(shared).toContain("a bare `dN` from another seat is read as a question back to whoever sent it");
  expect(shared).toContain("never resolved against whichever store you are standing in");
  expect(shared).toContain("A wrong-store hit returns a real, locked, plausible ruling instead of an error");
});

const packSection = async (name: string) => {
  const pack = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md")).text();
  return (new RegExp(`<!-- slp:${name}:begin -->([\\s\\S]*?)<!-- slp:${name}:end -->`).exec(pack)?.[1] ?? "")
    .replaceAll(/\s+/g, " ");
};
const resourceText = async (...path: string[]) =>
  (await Bun.file(join(import.meta.dir, "..", "src", "plugins", ...path)).text()).replaceAll(/\s+/g, " ");

test("slp-article objection [lint] an evidence-backed premise objection travels on --blocked and is ruled on either way", async () => {
  const shared = await packSection("shared");
  expect(shared).toContain("The same note carries an objection");
  expect(shared).toContain("a doubt it cannot yet back is a plain note and it proceeds");
  expect(shared).toContain("The right to object is not a duty to find one");
  expect(shared).toContain("rules on it either way with `maestro decide \"<keep|change>\" --why \"<reason>\" --work <id>`");
  expect(shared).toContain("changing the plan needs evidence and keeping it needs a reason");
  expect(shared).toContain("`maestro work accept <id> --outcome cancelled`");
  expect(shared).toContain("ruling that changes what another item rests on is recorded on that item too");
  expect(shared).toContain("A ruling that keeps the item's goal and acceptance and only relaxes a constraint or replaces a chosen design is an instruction to the holder, which continues the item under it");
  expect(shared).toContain("When the goal or the acceptance itself falls, the holder returns what it has");

  const peerBlock = await packSection("shared:peer");
  expect(peerBlock).toContain("or evidence that the request rests on something false");
  expect(peerBlock).toContain("The answer arrives as a `DECIDED` line on the item");

  const peer = await resourceText("resources", "profiles", "peer.md");
  expect(peer).toContain("\"<the premise, the evidence, and what the request would become>\" --blocked");
  expect(peer).toContain("A doubt you cannot yet back is a plain `maestro work note");
  expect(peer).toContain("Nothing obliges you to find an objection");
  expect(peer).not.toContain("proceed or wait as your own judgment says");
});

test("slp-article brief [lint] an objective tells the goal, its sourced constraints and the chosen design apart", async () => {
  const shared = await packSection("shared");
  expect(shared).toContain("so its reader can tell three things apart: the goal it serves");
  expect(shared).toContain("the constraints that bind it, each with where it comes from");
  expect(shared).toContain("named as a choice the reader may question with evidence");
  const lead = await resourceText("resources", "profiles", "lead.md");
  expect(lead).toContain("what to build and the goal it serves, the constraints and any chosen design told apart as the shared contract says");
  const peer = await resourceText("resources", "profiles", "peer.md");
  expect(peer).toContain("A constraint it traces to the owner or to a decision binds you");
  expect(peer).toContain("is a premise like any other and you may question it with evidence");
});

test("slp-article parachute [lint] ANTI_PATTERN.md names the parachute and pre-solve covers the fixed review brief", async () => {
  const text = await resourceText("resources", "ANTI_PATTERN.md");
  expect(text).toContain("## parachute A design choice made by one seat travels down later briefs as if it were a requirement");
  expect(text).toContain("https://vhlam.com/article/agent-orchestration-multi-agent-slp");
  expect(text).toContain("a review brief that fixes the hypothesis, the criterion and the answer shape in advance");
  expect(await resourceText("resources", "WORKFLOW.md")).toContain("holds the eight named");
  expect(await resourceText("skills", "maestro-improve", "SKILL.md")).toContain("one of the eight in `~/maestro/ANTI_PATTERN.md`");
});

test("slp-article subtraction [lint] the improver may delete and judges a mechanism by the outcomes it changed", async () => {
  const skill = await resourceText("skills", "maestro-improve", "SKILL.md");
  expect(skill).toContain("The smallest edit may be a deletion.");
  expect(skill).toContain("earns its place by the outcomes it changed, not by how often it ran");
  expect(skill).toContain("remove it rather than add a counterweight");
});

test("slp-article evidence [lint] a return names the state its evidence was taken on and a reviewer accepts only that state", async () => {
  const shared = await packSection("shared");
  expect(shared).toContain("A return names the state its evidence was taken on");
  expect(shared).toContain("a measurement whose conditions differed between runs");
  expect(shared).toContain("a message that a resource is free is not evidence that it is");
  const peerBlock = await packSection("shared:peer");
  expect(peerBlock).toContain("the evidence that it works and the commit or diff it was taken on");
  const peer = await resourceText("resources", "profiles", "peer.md");
  expect(peer).toContain("what proves it and on which state of the code it was proven");
});

test("slp-article hub-escalation [lint] the Hub section tells an unnamed Hub where a waiting escalation shows", async () => {
  const hub = await packSection("role:hub-supervisor");
  expect(hub).toContain("every escalation waiting on you as `blocked for the Hub: <team-id>:<id> <note>`");
});
