import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("122 decision draft stores a rationale body and show renders it", async () => {
  await withFixture(async (fixture) => {
    const created = await runCli(fixture, [
      "decision",
      "draft",
      "scaffold dir is .maestro/bundle/<id>",
      "--rationale",
      "keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    ]);
    expect(created.exitCode).toBe(0);
    const id = idFrom(created);
    expect(created.stdout).toContain(
      "rationale: keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    );

    const shown = await runCli(fixture, ["decision", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(
      "rationale: keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    );

    const help = await runCli(fixture, ["help", "decision"]);
    expect(help.stdout).toContain("--rationale");
  });
});

test("123 decision without rationale stays valid and renders no rationale line", async () => {
  await withFixture(async (fixture) => {
    const created = await runCli(fixture, ["decision", "draft", "plain decision"]);
    expect(created.exitCode).toBe(0);
    const shown = await runCli(fixture, ["decision", "show", idFrom(created)]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).not.toContain("rationale:");
  });
});
