import { expect, test } from "bun:test";
import { readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  initializeGitRepository,
  runCli,
  runTool,
  withFixture,
} from "./helpers.ts";

const placeholder = "<!-- handoff: unfilled -->";

test("168 handoff renders every store-provable NOTES section", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const done = idFrom(
      await runCli(fixture, ["work", "add", "completed packet work", "--atomic-reason", "fixture"]),
    );
    const open = idFrom(
      await runCli(fixture, ["work", "add", "open packet work", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", done])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", done, "--evidence", "tests 2/2"])).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "note", done, "failed: first path"])).exitCode)
      .toBe(0);
    expect((await runCli(fixture, ["work", "note", open, "failed: second path"])).exitCode)
      .toBe(0);
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "keep the packet factual",
        "--rationale",
        "store evidence only",
        "--work",
        done,
      ]),
    );
    expect((await runCli(fixture, ["decision", "lock", decision])).exitCode).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "bundle",
          "open",
          "handoff-render",
          "--work",
          done,
          "--work",
          open,
        ])
      ).exitCode,
    ).toBe(0);
    const baseHead = (await runTool(["git", "rev-parse", "--short", "HEAD"], fixture.repo)).stdout
      .trim();

    await writeFile(join(fixture.repo, "base-marker.txt"), "new base\n");
    expect((await runTool(["git", "add", "base-marker.txt"], fixture.repo)).exitCode).toBe(0);
    expect(
      (
        await runTool(
          [
            "git",
            "-c",
            "user.name=Maestro Tests",
            "-c",
            "user.email=maestro-tests@example.invalid",
            "commit",
            "-m",
            "advance base",
          ],
          fixture.repo,
        )
      ).exitCode,
    ).toBe(0);
    const head = (await runTool(["git", "rev-parse", "--short", "HEAD"], fixture.repo)).stdout
      .trim();

    const rendered = await runCli(fixture, ["handoff", "handoff-render", "--json"]);
    expect(rendered.exitCode).toBe(0);
    const envelope = JSON.parse(rendered.stdout) as {
      data: { leftAlone: string[]; written: string[] };
      ok: boolean;
    };
    expect(envelope.ok).toBe(true);
    expect(envelope.data.written).toEqual([
      "Current State",
      "Next Action",
      "Authority",
      "Failed approaches",
      "Do not repeat",
    ]);
    expect(envelope.data.leftAlone).toEqual(["Base"]);

    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "handoff-render", "NOTES.md"),
    ).text();
    expect(notes).toContain(`Base: ${baseHead} (main)`);
    expect(notes).not.toContain(`Base: ${head} (main)`);
    expect(notes).toContain(`- ${done} [done] completed packet work\n  evidence: tests 2/2`);
    expect(notes).toContain(`- ${open} [open] open packet work\n  evidence: none recorded`);
    expect(notes).toContain(`- ${decision} [locked] keep the packet factual`);
    expect(notes.indexOf(`${done}: failed: first path`)).toBeLessThan(
      notes.indexOf(`${open}: failed: second path`),
    );
    for (const section of ["Next Action", "Authority", "Do not repeat"]) {
      expect(notes).toContain(`## ${section}\n\n${placeholder}`);
    }
  });
});

test("169 handoff preserves human sections and leaves its own second run byte-identical", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    expect((await runCli(fixture, ["bundle", "open", "handoff-idempotent"])).exitCode).toBe(0);
    const notesPath = join(
      fixture.repo,
      ".maestro",
      "bundle",
      "handoff-idempotent",
      "NOTES.md",
    );
    const scaffold = await Bun.file(notesPath).text();
    await writeFile(
      notesPath,
      scaffold.replace("## Next Action\n\n## Authority", "## Next Action\n\nRun the release audit.\n\n## Authority"),
    );

    const first = await runCli(fixture, ["handoff", "handoff-idempotent"]);
    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain("left alone: Base, Next Action");
    const afterFirst = await Bun.file(notesPath).text();
    expect(afterFirst).toContain("## Next Action\n\nRun the release audit.");

    const second = await runCli(fixture, ["handoff", "handoff-idempotent"]);
    expect(second.exitCode).toBe(0);
    expect(await Bun.file(notesPath).text()).toBe(afterFirst);
    expect(second.stdout).toContain(
      "left alone: Base, Current State, Next Action, Authority, Failed approaches, Do not repeat",
    );
  });
});

test("170 bundle close refuses handoff placeholders and passes after replacement", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    expect((await runCli(fixture, ["bundle", "open", "handoff-gate"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["handoff", "handoff-gate"])).exitCode).toBe(0);

    const blocked = await runCli(fixture, ["bundle", "close", "handoff-gate"]);
    expect(blocked.exitCode).not.toBe(0);
    const error = JSON.parse(blocked.stderr) as {
      error: { code: string; command: string; message: string };
    };
    expect(error.error.code).toBe("HANDOFF_INCOMPLETE");
    expect(error.error.command).toBe("maestro bundle close handoff-gate");
    expect(error.error.message).toContain("replace every handoff placeholder");

    const notesPath = join(fixture.repo, ".maestro", "bundle", "handoff-gate", "NOTES.md");
    await writeFile(notesPath, (await Bun.file(notesPath).text()).replaceAll(placeholder, "filled by human"));
    const closed = await runCli(fixture, ["bundle", "close", "handoff-gate"]);
    expect(closed.exitCode).toBe(0);
    expect(closed.stdout).toContain("handoff-gate [archived]");
  });
});

test("171 handoff rejects work ids and unknown bundles with the next command", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "not a bundle", "--atomic-reason", "fixture"]),
    );
    const workTarget = await runCli(fixture, ["handoff", work]);
    expect(workTarget.exitCode).not.toBe(0);
    const workError = JSON.parse(workTarget.stderr) as {
      error: { code: string; command: string; message: string };
    };
    expect(workError.error.code).toBe("INVALID_TARGET");
    expect(workError.error.command).toBe("maestro bundle list");
    expect(workError.error.message).toContain(`${work} is a work id`);

    const missing = await runCli(fixture, ["handoff", "missing-bundle"]);
    expect(missing.exitCode).not.toBe(0);
    const missingError = JSON.parse(missing.stderr) as {
      error: { command: string; message: string };
    };
    expect(missingError.error.command).toBe("maestro bundle list");
    expect(missingError.error.message).toContain("bundle not found: missing-bundle");
  });
});

test("172 the four-skill roster points handoffs at maestro handoff", async () => {
  const root = join(import.meta.dir, "..", "src", "plugins", "skills");
  const roster = (await readdir(root, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  expect(roster).toEqual(["maestro-bundle", "maestro-design", "maestro-verify", "maestro-work"]);

  const skill = await Bun.file(join(root, "maestro-bundle", "SKILL.md")).text();
  expect(skill).toContain("Run `maestro handoff <bundle-id>` to seed untouched NOTES.md sections");
});
