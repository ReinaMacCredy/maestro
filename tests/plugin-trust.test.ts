import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  runCli,
  withFixture,
  writeConfig,
  writeUntrustedPlugin,
  type Fixture,
} from "./helpers.ts";

// Asserting that a verb failed to register is not proof: apply() can be skipped
// while module scope still ran. Only the absence of this file proves the import
// never happened.
function tripwire(sentinel: string, name: string): string {
  return `
import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(sentinel)}, "ran");
export default {
  name: ${JSON.stringify(name)},
  apply(ctx) {
    ctx.effect(() => ctx.cli.register(${JSON.stringify(name)}, async () => "tripwire ran"));
  },
};
`;
}

function sentinelPath(fixture: Fixture, name = "sentinel"): string {
  return join(fixture.home, name);
}

test("507 an untrusted repo plugin never reaches module scope, and listing it executes nothing", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.exitCode).toBe(0);
    expect(existsSync(sentinel)).toBe(false);
    expect(listed.stdout).toContain("tripwire");
    expect(listed.stdout).toContain("untrusted");

    // The auto-trigger: a harness hook reaches the loader with no user command.
    const hooked = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
    expect(hooked.exitCode).toBe(0);
    expect(existsSync(sentinel)).toBe(false);

    const verb = await runCli(fixture, ["tripwire"]);
    expect(verb.exitCode).not.toBe(0);
    expect(existsSync(sentinel)).toBe(false);
  });
});

test("508 repository config cannot grant itself trust", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));
    await writeConfig(fixture, [{ name: "tripwire", disabled: false }]);

    const listed = await runCli(fixture, ["plugin", "list"]);

    expect(listed.stdout).toContain("untrusted");
    expect(existsSync(sentinel)).toBe(false);
  });
});

test("509 plugin trust opens the gate and plugin enable never confers it", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));

    const enabled = await runCli(fixture, ["plugin", "enable", "tripwire"]);
    expect(enabled.exitCode).not.toBe(0);
    expect(enabled.stderr).toContain("plugin trust");
    expect(existsSync(sentinel)).toBe(false);

    const trusted = await runCli(fixture, ["plugin", "trust", "tripwire"]);
    expect(trusted.exitCode).toBe(0);
    expect(trusted.stdout).toContain("sha256:");
    // Trusting records a grant; it does not itself import.
    expect(existsSync(sentinel)).toBe(false);

    const verb = await runCli(fixture, ["tripwire"]);
    expect(verb.exitCode).toBe(0);
    expect(verb.stdout).toContain("tripwire ran");
    expect(existsSync(sentinel)).toBe(true);
  });
});

test("510 replacing a trusted plugin's source revokes the grant", async () => {
  await withFixture(async (fixture) => {
    const first = sentinelPath(fixture, "first");
    const second = sentinelPath(fixture, "second");
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(first, "tripwire"));
    await runCli(fixture, ["plugin", "trust", "tripwire"]);
    await runCli(fixture, ["tripwire"]);
    expect(existsSync(first)).toBe(true);

    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(second, "tripwire"));

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.stdout).toContain("untrusted");
    expect(existsSync(second)).toBe(false);
  });
});

test("511 a directory plugin's digest covers files the entrypoint imports, not just the entrypoint", async () => {
  await withFixture(async (fixture) => {
    const first = sentinelPath(fixture, "first");
    const second = sentinelPath(fixture, "second");
    const directory = join(fixture.repo, ".maestro", "plugins", "bundle");
    await mkdir(directory, { recursive: true });
    await writeFile(
      join(directory, "index.ts"),
      `
import "./helper.ts";
export default { name: "bundle", apply() {} };
`,
    );
    await writeFile(join(directory, "helper.ts"), `
import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(first)}, "ran");
`);

    await runCli(fixture, ["plugin", "trust", "bundle"]);
    await runCli(fixture, ["plugin", "list"]);
    expect(existsSync(first)).toBe(true);

    // Only the sibling changes; the entrypoint is byte-identical.
    await writeFile(join(directory, "helper.ts"), `
import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(second)}, "ran");
`);

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.stdout).toContain("untrusted");
    expect(existsSync(second)).toBe(false);
  });
});

test("512 install grandfathers the home tier only, and never a repository plugin", async () => {
  await withFixture(async (fixture) => {
    const mine = sentinelPath(fixture, "mine");
    const theirs = sentinelPath(fixture, "theirs");
    await writeUntrustedPlugin(fixture, "global", "mine", tripwire(mine, "mine"));
    await writeUntrustedPlugin(fixture, "repo", "theirs", tripwire(theirs, "theirs"));

    const { grandfatherHomePlugins } = await import("../src/plugins/plugin-trust.ts");
    const grandfathered = await grandfatherHomePlugins(fixture.home);
    expect(grandfathered).toEqual(["mine"]);

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.stdout).toContain("mine\tglobal\tactive");
    expect(listed.stdout).toContain("theirs\trepo\tuntrusted");
    expect(existsSync(mine)).toBe(true);
    // The clone-supplied one is exactly what the boundary defends against.
    expect(existsSync(theirs)).toBe(false);
  });
});

test("513 grandfathering runs once and never resurrects a withdrawn grant", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "global", "mine", tripwire(sentinel, "mine"));
    const { grandfatherHomePlugins } = await import("../src/plugins/plugin-trust.ts");

    await grandfatherHomePlugins(fixture.home);
    expect((await runCli(fixture, ["plugin", "untrust", "mine"])).exitCode).toBe(0);

    expect(await grandfatherHomePlugins(fixture.home)).toEqual([]);

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.stdout).toContain("untrusted");
  });
});

test("514 plugin remove drops the grant so a later file at that path is not vouched for", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));
    await runCli(fixture, ["plugin", "trust", "tripwire"]);
    expect((await runCli(fixture, ["plugin", "remove", "tripwire"])).exitCode).toBe(0);

    // A byte-identical plugin returns to the same path; the old grant must not
    // still cover it. The sentinel is cleared first because removing a trusted
    // plugin loads it one last time on the way out.
    await rm(sentinel, { force: true });
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));
    const listed = await runCli(fixture, ["plugin", "list"]);

    expect(listed.stdout).toContain("untrusted");
    expect(existsSync(sentinel)).toBe(false);
  });
});

test("515 an untrusted plugin named by repo config lists once, not also as a missing source", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture);
    await writeUntrustedPlugin(fixture, "repo", "tripwire", tripwire(sentinel, "tripwire"));
    await writeConfig(fixture, [{ name: "tripwire", disabled: false }]);

    const rows = (await runCli(fixture, ["plugin", "list"])).stdout
      .split("\n")
      .filter((line) => line.startsWith("tripwire\t"));

    // The source is found; it is untrusted. Reporting it as missing as well
    // sends the reader looking for a file that is right there.
    expect(rows).toEqual(["tripwire\trepo\tuntrusted"]);
  });
});

test("516 a trusted plugin cannot import code from outside its artifact", async () => {
  await withFixture(async (fixture) => {
    const sentinel = sentinelPath(fixture, "escaped");
    const directory = join(fixture.repo, ".maestro", "plugins", "bundle");
    await mkdir(directory, { recursive: true });
    await writeFile(
      join(directory, "index.ts"),
      `
import "../../../helper.ts";
export default { name: "bundle", apply() {} };
`,
    );
    // Outside the artifact, so the digest never covers it: editing this file
    // would change what a granted plugin runs while the grant still matched.
    await writeFile(join(fixture.repo, "helper.ts"), `
import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(sentinel)}, "ran");
`);

    const trusted = await runCli(fixture, ["plugin", "trust", "bundle"]);
    expect(trusted.exitCode).toBe(0);

    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(existsSync(sentinel)).toBe(false);
    expect(listed.stdout).toContain("bundle\trepo\terror");
    expect(listed.stdout).toContain("index.ts");
    expect(listed.stdout).toContain('"../../../helper.ts"');
  });
});

test("517 a trusted plugin still imports a sibling inside its artifact", async () => {
  await withFixture(async (fixture) => {
    const directory = join(fixture.repo, ".maestro", "plugins", "kit");
    await mkdir(directory, { recursive: true });
    await writeFile(join(directory, "index.ts"), `
import { greeting } from "./sibling.ts";
export default {
  name: "kit",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register("kit", async () => greeting));
  },
};
`);
    await writeFile(join(directory, "sibling.ts"), `export const greeting = "sibling loaded";\n`);

    const trusted = await runCli(fixture, ["plugin", "trust", "kit"]);
    expect(trusted.exitCode).toBe(0);

    const ran = await runCli(fixture, ["kit"]);
    expect(ran.exitCode).toBe(0);
    expect(ran.stdout).toContain("sibling loaded");
  });
});
