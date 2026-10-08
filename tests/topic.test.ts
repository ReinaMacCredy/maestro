import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { realpath, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("topic path creates a git-ignored folder under .maestro/topic", async () => {
  await withFixture(async (fixture) => {
    Bun.spawnSync(["git", "init", "-q"], { cwd: fixture.repo });
    Bun.spawnSync(["git", "add", "-A"], { cwd: fixture.repo });
    Bun.spawnSync(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"], {
      cwd: fixture.repo,
    });

    const result = await runCli(fixture, ["topic", "path", "sidebar"]);
    expect(result.exitCode).toBe(0);
    const directory = join(fixture.repo, ".maestro", "topic", "sidebar");
    expect(result.stdout.trim()).toBe(join(await realpath(fixture.repo), ".maestro", "topic", "sidebar"));
    expect(existsSync(directory)).toBe(true);

    await writeFile(join(directory, "demo.html"), "<html></html>");
    const status = Bun.spawnSync(["git", "status", "--porcelain", "--untracked-files=all"], {
      cwd: fixture.repo,
    });
    expect(status.stdout.toString()).not.toContain(".maestro/topic");
  });
});

test("topic path refuses a slug that leaves the topic folder", async () => {
  await withFixture(async (fixture) => {
    for (const slug of ["../escape", "a/b", "."]) {
      const result = await runCli(fixture, ["topic", "path", slug]);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("INVALID_TOPIC_SLUG");
    }
    expect(existsSync(join(fixture.repo, "escape"))).toBe(false);
  });
});

test("topic list names each topic with its files", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["topic", "list"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout).toContain("no topics");

    await runCli(fixture, ["topic", "path", "sidebar"]);
    const directory = join(fixture.repo, ".maestro", "topic", "sidebar");
    await writeFile(join(directory, "demo.html"), "<html></html>");
    await writeFile(join(directory, "shot.png"), "");
    await writeFile(join(directory, "notes.md"), "");

    const listed = await runCli(fixture, ["topic", "list"]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("sidebar");
    expect(listed.stdout).toContain("demo.html");
    expect(listed.stdout).not.toContain(".gitignore");

    const json = await runCli(fixture, ["topic", "list", "--json"]);
    const parsed = JSON.parse(json.stdout) as { data: { topics: { slug: string; files: string[] }[] } };
    expect(parsed.data.topics).toEqual([
      expect.objectContaining({ slug: "sidebar", files: ["demo.html", "notes.md", "shot.png"] }),
    ]);
  });
});
