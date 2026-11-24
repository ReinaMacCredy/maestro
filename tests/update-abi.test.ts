import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { withFixture } from "./helpers.ts";

const plugins = join(import.meta.dir, "..", "src", "plugins");
const href = (file: string) => JSON.stringify(pathToFileURL(join(plugins, file)).href);

test("the post-swap scripts of the released update still run against this tree (v0.126.0 lifecycle.ts:339, :362)", async () => {
  await withFixture(async (fixture) => {
    const run = async (script: string) => {
      const child = Bun.spawn([process.execPath, "-e", script], {
        cwd: fixture.repo,
        env: { ...process.env, HOME: fixture.home },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [exitCode, stderr] = await Promise.all([child.exited, new Response(child.stderr).text()]);
      return { exitCode, stderr };
    };
    const home = JSON.stringify(fixture.home);
    const skills = await run(
      `const skills = await import(${href("skills.ts")});` +
        `const sync = await skills.materializeSkills(${home}, "0000000000000000000000000000000000000000");` +
        `console.log(skills.formatSkillSync(sync));`,
    );
    expect(skills).toEqual({ exitCode: 0, stderr: "" });
    const room = await run(
      `const room = await import(${href("room.ts")});` +
        `await room.scaffoldRoom(${home});` +
        `const profiles = await import(${href("profiles.ts")});` +
        `await profiles.materializeProfiles(${home}, ${JSON.stringify(fixture.repo)});`,
    );
    expect(room).toEqual({ exitCode: 0, stderr: "" });
    expect(existsSync(join(fixture.home, "maestro", "skills"))).toBe(true);
  });
}, 60_000);
