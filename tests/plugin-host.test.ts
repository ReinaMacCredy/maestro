import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import {
  helloPlugin,
  idFrom,
  runCli,
  runTool,
  setPlugin,
  withFixture,
  writeConfig,
  writePlugin,
} from "./helpers.ts";

test("1 plugin-registered effect dispatches its verb on the next invocation", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(fixture, "global", "hello", helloPlugin);

    const result = await runCli(fixture, ["hello"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("hello from effect");
  });
});

test("2 a disabled plugin contributes neither verbs nor gates", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(
      fixture,
      "repo",
      "stopper",
      `
export default {
  name: "stopper",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register("hello", async () => "should not run"));
    ctx.effect(() => ctx.events.on("work.start", async () => ({
      blocked: true,
      origin: "stopper",
      reason: "should not run",
    })));
  },
};
`,
    );
    await writeConfig(fixture, [{ name: "stopper", disabled: true }]);

    const unknown = await runCli(fixture, ["hello"]);
    const added = await runCli(fixture, ["work", "add", "read only", "--kind", "idea"]);
    const started = await runCli(fixture, ["work", "start", idFrom(added)]);

    expect(unknown.exitCode).not.toBe(0);
    expect(unknown.stderr).toContain("unknown verb");
    expect(started.exitCode).toBe(0);
    expect(started.stderr).not.toContain("stopper");
  });
});

test("3 a repo-local TypeScript plugin loads without build or install", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(fixture, "repo", "hello", helloPlugin);

    const result = await runCli(fixture, ["hello"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("hello from effect");
  });
});

test("4 a missing injected service isolates that plugin and reports the dependency", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(
      fixture,
      "repo",
      "broken",
      `
export default {
  name: "broken",
  inject: ["nosuch"],
  apply() {
    throw new Error("must not apply");
  },
};
`,
    );
    await writePlugin(fixture, "repo", "hello", helloPlugin);

    const listed = await runCli(fixture, ["plugin", "list"]);
    const healthy = await runCli(fixture, ["hello"]);

    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("broken");
    expect(listed.stdout).toContain("nosuch");
    expect(healthy.exitCode).toBe(0);
  });
});

test("5 disabling an enabled plugin unwinds all of its effects", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(fixture, "repo", "hello", helloPlugin);
    await setPlugin(fixture, "hello", true);

    const enabled = await runCli(fixture, ["plugin", "enable", "hello"]);
    const present = await runCli(fixture, ["hello"]);
    const disabled = await runCli(fixture, ["plugin", "disable", "hello"]);
    const absent = await runCli(fixture, ["hello"]);

    expect(enabled.exitCode).toBe(0);
    expect(present.exitCode).toBe(0);
    expect(disabled.exitCode).toBe(0);
    expect(absent.exitCode).not.toBe(0);
    expect(absent.stderr).toContain("unknown verb");
  });
});

test("6 plugin new and plugin add appear with repo and global source tiers", async () => {
  await withFixture(async (fixture) => {
    const remote = join(fixture.root, "remote-gate");
    await mkdir(remote, { recursive: true });
    await writeFile(
      join(remote, "index.ts"),
      `export default { name: "remote-gate", apply() {} };\n`,
    );
    expect((await runTool(["git", "init", "-q"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.email", "test@example.invalid"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.name", "Stage One"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "add", "index.ts"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "commit", "-qm", "init"], remote)).exitCode).toBe(0);

    const created = await runCli(fixture, ["plugin", "new", "my-gate"]);
    const added = await runCli(fixture, ["plugin", "add", remote]);
    const listed = await runCli(fixture, ["plugin", "list"]);

    expect(created.exitCode).toBe(0);
    expect(added.exitCode).toBe(0);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("my-gate");
    expect(listed.stdout).toContain("repo");
    expect(listed.stdout).toContain(basename(remote));
    expect(listed.stdout).toContain("global");

    await writePlugin(
      fixture,
      "repo",
      "index",
      `export default { name: "index-local", apply() {} };\n`,
    );
    const removed = await runCli(fixture, ["plugin", "remove", "index-local"]);
    const scaffoldStillLoads = await runCli(fixture, ["my-gate"]);
    expect(removed.exitCode).toBe(0);
    expect(scaffoldStillLoads.exitCode).toBe(0);
  });
});

test("26 plugin enable refuses an unresolvable source without changing config", async () => {
  await withFixture(async (fixture) => {
    const configPath = join(fixture.repo, ".maestro", "config");
    const before = await readFile(configPath, "utf8");

    const enabled = await runCli(fixture, ["plugin", "enable", "policy-tdd"]);

    expect(enabled.exitCode).not.toBe(0);
    expect(enabled.stderr).toContain("policy-tdd");
    expect(enabled.stderr).toContain("source");
    expect(await readFile(configPath, "utf8")).toBe(before);
  });
});

test("27 plugin add rejects missing entrypoints and immediately lists loadable clones", async () => {
  await withFixture(async (fixture) => {
    const invalid = join(fixture.root, "invalid-plugin");
    await mkdir(invalid, { recursive: true });
    await writeFile(join(invalid, "README.md"), "no TypeScript entrypoint\n");
    expect((await runTool(["git", "init", "-q"], invalid)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.email", "test@example.invalid"], invalid)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.name", "Stage One"], invalid)).exitCode).toBe(0);
    expect((await runTool(["git", "add", "README.md"], invalid)).exitCode).toBe(0);
    expect((await runTool(["git", "commit", "-qm", "invalid"], invalid)).exitCode).toBe(0);

    const rejected = await runCli(fixture, ["plugin", "add", invalid]);
    const rejectedClone = join(fixture.home, ".maestro", "plugins", basename(invalid));

    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("entrypoint");
    expect(existsSync(rejectedClone)).toBe(false);

    const valid = join(fixture.root, "valid-plugin");
    await mkdir(valid, { recursive: true });
    await writeFile(
      join(valid, "index.ts"),
      `export default { name: "valid-plugin", apply() {} };\n`,
    );
    expect((await runTool(["git", "init", "-q"], valid)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.email", "test@example.invalid"], valid)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.name", "Stage One"], valid)).exitCode).toBe(0);
    expect((await runTool(["git", "add", "index.ts"], valid)).exitCode).toBe(0);
    expect((await runTool(["git", "commit", "-qm", "valid"], valid)).exitCode).toBe(0);

    const added = await runCli(fixture, ["plugin", "add", valid]);
    const listed = await runCli(fixture, ["plugin", "list"]);
    const config = JSON.parse(await readFile(join(fixture.repo, ".maestro", "config"), "utf8")) as {
      plugins: Array<{ name: string }>;
    };

    expect(added.exitCode).toBe(0);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("valid-plugin");
    expect(listed.stdout).toContain("global");
    expect(config.plugins.map((entry) => entry.name)).toContain("valid-plugin");
  });
});

test("28 plugin remove deletes scaffolded and added artifacts plus config entries", async () => {
  await withFixture(async (fixture) => {
    const localName = "remove-local";
    const localPath = join(fixture.repo, ".maestro", "plugins", `${localName}.ts`);
    expect((await runCli(fixture, ["plugin", "new", localName])).exitCode).toBe(0);
    expect((await runCli(fixture, ["plugin", "disable", localName])).exitCode).toBe(0);

    const remote = join(fixture.root, "remove-global");
    await mkdir(remote, { recursive: true });
    await writeFile(
      join(remote, "index.ts"),
      `export default { name: "remove-global", apply() {} };\n`,
    );
    expect((await runTool(["git", "init", "-q"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.email", "test@example.invalid"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "config", "user.name", "Stage One"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "add", "index.ts"], remote)).exitCode).toBe(0);
    expect((await runTool(["git", "commit", "-qm", "remove"], remote)).exitCode).toBe(0);
    expect((await runCli(fixture, ["plugin", "add", remote])).exitCode).toBe(0);

    const globalPath = join(fixture.home, ".maestro", "plugins", "remove-global");
    expect(existsSync(localPath)).toBe(true);
    expect(existsSync(globalPath)).toBe(true);

    const removedLocal = await runCli(fixture, ["plugin", "remove", localName]);
    const removedGlobal = await runCli(fixture, ["plugin", "remove", "remove-global"]);
    const config = JSON.parse(await readFile(join(fixture.repo, ".maestro", "config"), "utf8")) as {
      plugins: Array<{ name: string }>;
    };

    expect(removedLocal.exitCode).toBe(0);
    expect(removedGlobal.exitCode).toBe(0);
    expect(existsSync(localPath)).toBe(false);
    expect(existsSync(globalPath)).toBe(false);
    expect(config.plugins.map((entry) => entry.name)).not.toContain(localName);
    expect(config.plugins.map((entry) => entry.name)).not.toContain("remove-global");
  });
});
