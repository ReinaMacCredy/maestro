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
  const observerMode = await Bun.file(join(docsRoot, "guides", "observer-mode.md")).text();
  const scenarios = await Bun.file(join(docsRoot, "guides", "slp-scenarios.md")).text();
  const reference = await Bun.file(join(docsRoot, "reference", "cli.md")).text();
  const combined = [setup, guide, roles, lanes, observerMode, scenarios, reference].join("\n");

  for (const command of slpOperations) expect(combined).toContain(command);
  expect(guide).toContain("there is no Observer, Advisor, scheduler, health or reconcile");
  expect(guide).toContain("foreground Watch Pane");
  expect(guide).toContain("not an agent");
  expect(guide).toContain("transcript is runtime-only and is deleted at stop.");
  expect(setup).toContain("~/maestro/SLP.md");
  expect(setup).toContain("<project>/.maestro/SLP.md");
  expect(scenarios).not.toContain("herdr workspace create --cwd ~/Code/rewrite");
  for (const documented of ["### `term`", "### `memory`", "--local", "HUB_UNAVAILABLE", "`import <dir> [--dry-run]`"]) {
    expect(reference).toContain(documented);
  }
  expect(observerMode).toContain("--local");

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

test("576 [lint] no doctrine, template, or skill demands a test or SPEC from quickfix or Light work", async () => {
  // Anti-goal A6 / d784: ceremony scales with tier. Proves the shipped text and
  // the scaffolded SPEC, not how a harness reads them.
  const root = join(import.meta.dir, "..", "src", "plugins");
  // A sentence that denies the demand ("no SPEC", "never demand a test") is
  // the rule itself, not a demand; strip those clauses before matching.
  const denial = /\b(no|never|without|nor)\b[^.;]*?\b(SPEC|red[- ]tests?( list)?|red list|tests?|VERIFY\.md)\b[^.;]*/gi;
  const demand = /\b(red[- ]tests?|red list|SPEC|VERIFY\.md|failing test|write (a|the) test)\b/i;
  const demands = (text: string): boolean => demand.test(text.replace(denial, ""));
  const section = (text: string, heading: string): string => {
    const start = text.indexOf(heading);
    expect({ heading, found: start >= 0 }).toEqual({ heading, found: true });
    const rest = text.slice(start + heading.length);
    const next = rest.search(/\n## /);
    return next >= 0 ? rest.slice(0, next) : rest;
  };
  const bullets = (text: string, prefix: string): string[] => {
    const lines = text.split("\n");
    const out: string[] = [];
    for (let index = 0; index < lines.length; index += 1) {
      if (!lines[index]?.startsWith(prefix)) continue;
      const item = [lines[index]];
      while (lines[index + 1]?.startsWith("  ")) item.push(lines[++index] ?? "");
      out.push(item.join("\n"));
    }
    return out;
  };
  const lowTierBullets = (text: string, prefixes: string[]): string[] =>
    prefixes.flatMap((prefix) => bullets(text, prefix));

  const bundle = await Bun.file(join(root, "skills", "maestro-bundle", "SKILL.md")).text();
  const tierRule = section(bundle, "## Tier rule");
  const work = await Bun.file(join(root, "skills", "maestro-work", "SKILL.md")).text();
  const tierFirst = section(work, "## Tier first, recon second");
  const workflow = await Bun.file(join(root, "resources", "WORKFLOW.md")).text();
  const tiers = section(workflow, "## Tiers");
  const surfaces: Array<[string, string[]]> = [
    [tierRule, lowTierBullets(tierRule, ["- quickfix:", "- Light:"])],
    [tierFirst, lowTierBullets(tierFirst, ["- quickfix:", "- Light:"])],
    [tiers, lowTierBullets(tiers, ["- **quickfix**", "- **Light"])],
  ];
  for (const [, low] of surfaces) {
    expect(low).toHaveLength(2);
    for (const bullet of low) {
      expect({ bullet, demands: demands(bullet) }).toEqual({ bullet, demands: false });
      expect(bullet).toMatch(/inline|no record|no bundle/i);
    }
  }
  expect(tierRule).toContain("Quickfix and Light never demand a SPEC or a test");
  expect(tiers).toContain("Red tests are a Full-tier instrument");

  const design = await Bun.file(join(root, "skills", "maestro-design", "SKILL.md")).text();
  const exit = section(design, "## Readiness gate and exit");
  const lightExit = bullets(exit, "- Light:")[0] ?? "";
  expect({ lightExit, demands: demands(lightExit) }).toEqual({ lightExit, demands: false });

  await withFixture(async (fixture) => {
    const opened = await runCli(fixture, ["bundle", "open", "tier-lint"]);
    expect(opened.exitCode).toBe(0);
    const spec = await Bun.file(join(fixture.repo, ".maestro", "bundle", "tier-lint", "SPEC.md")).text();
    const redTests = section(spec, "## Red tests");
    expect(redTests).toContain("Full tier only");
    expect(redTests).toContain("Quickfix and Light work never carries this section");
    expect(section(spec, "## Decisions")).toContain("maestro bundle show tier-lint");
    expect(section(spec, "## Anti-goals")).toContain("matching VERIFY.md check");
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
  const record = grilling.slice(grilling.indexOf("- Record each answer the moment it lands"));
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
  expect(pack).toMatch(/no seat or process watches panes for stalls/);

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
