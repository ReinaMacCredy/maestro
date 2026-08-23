import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
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
  });
});
