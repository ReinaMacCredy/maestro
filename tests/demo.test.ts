import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { realpath, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("demo path creates a git-ignored folder under .maestro/demo", async () => {
  await withFixture(async (fixture) => {
    Bun.spawnSync(["git", "init", "-q"], { cwd: fixture.repo });
    Bun.spawnSync(["git", "add", "-A"], { cwd: fixture.repo });
    Bun.spawnSync(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"], {
      cwd: fixture.repo,
    });

    const result = await runCli(fixture, ["demo", "path", "sidebar"]);
    expect(result.exitCode).toBe(0);
    const directory = join(fixture.repo, ".maestro", "demo", "sidebar");
    expect(result.stdout.trim()).toBe(join(await realpath(fixture.repo), ".maestro", "demo", "sidebar"));
    expect(existsSync(directory)).toBe(true);

    await writeFile(join(directory, "demo.html"), "<html></html>");
    const status = Bun.spawnSync(["git", "status", "--porcelain", "--untracked-files=all"], {
      cwd: fixture.repo,
    });
    expect(status.stdout.toString()).not.toContain(".maestro/demo");
  });
});

test("demo path refuses a slug that leaves the demo folder", async () => {
  await withFixture(async (fixture) => {
    for (const slug of ["../escape", "a/b", "."]) {
      const result = await runCli(fixture, ["demo", "path", slug]);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("INVALID_DEMO_SLUG");
    }
    expect(existsSync(join(fixture.repo, "escape"))).toBe(false);
  });
});

test("demo list names each demo with its files", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["demo", "list"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout).toContain("no demos");

    await runCli(fixture, ["demo", "path", "sidebar"]);
    const directory = join(fixture.repo, ".maestro", "demo", "sidebar");
    await writeFile(join(directory, "demo.html"), "<html></html>");
    await writeFile(join(directory, "shot.png"), "");

    const listed = await runCli(fixture, ["demo", "list"]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("sidebar");
    expect(listed.stdout).toContain("demo.html");
    expect(listed.stdout).not.toContain(".gitignore");

    const json = await runCli(fixture, ["demo", "list", "--json"]);
    const parsed = JSON.parse(json.stdout) as { data: { demos: { slug: string; files: string[] }[] } };
    expect(parsed.data.demos).toEqual([
      expect.objectContaining({ slug: "sidebar", files: ["demo.html", "shot.png"] }),
    ]);
  });
});
