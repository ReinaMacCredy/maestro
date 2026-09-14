import { expect, test } from "bun:test";
import { join } from "node:path";
import { builtInPlugins } from "../src/plugins/index.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { skillNames } from "../src/plugins/skills.ts";
import { runCli, withFixture } from "./helpers.ts";

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
  const peer = await Bun.file(
    join(import.meta.dir, "..", "src", "plugins", "resources", "profiles", "peer.md"),
  ).text();
  expect(pack).not.toContain("slp:role:peer:begin");
  for (const allowed of ["status", "take assigned work", "notes", "return results"]) {
    expect(peer).toContain(allowed);
  }
  for (const forbidden of ["team start", "team stop", "accept Peer", "decide technical"]) {
    expect(peer).not.toContain(forbidden);
  }

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
    ...["bundle", "design", "work", "verify", "explore"].map((name) => `skills/maestro-${name}/SKILL.md`),
    "skills/maestro-design/references/grilling.md",
    "skills/maestro-design/references/domain-modeling.md",
    "skills/maestro-work/references/tdd-antipatterns.md",
    ...["design", "work", "ship"].map((name) => `recipes/${name}.md`),
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

test("docs-contract: pack v3 markers, no Observer, --peer-profile named and the retired flags absent, OWNER.md template names the three seat profiles (red 11, item 9)", async () => {
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
  for (const surface of [recipe, cli]) {
    expect(surface).toContain("--peer-profile");
    expect(surface).not.toContain("--lead-profile");
    expect(surface).not.toContain("--observer-model");
    expect(surface).not.toContain("sentinel");
  }
  expect(cli).not.toContain("--stall repeat");

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
  const peer = unwrap(await Bun.file(join(profiles, "peer.md")).text());
  expect(peer).not.toContain("Communicate directly");
  expect(peer).toContain("only through recorded work notes and returns");
  expect(unwrap(await Bun.file(join(profiles, "lead.md")).text())).toContain("Communicate directly with the Team Supervisor and every Peer");
  expect(unwrap(await Bun.file(join(profiles, "team-supervisor.md")).text())).toContain("Communicate directly with the Hub Supervisor, the Lead, and every Peer");

  const recipe = unwrap(await Bun.file(join(root, "src", "plugins", "recipes", "slp.md")).text());
  expect(recipe).not.toContain("The Team Supervisor, Lead, and Peers may talk directly");
  expect(recipe).toContain("recorded work notes and returns");
});
