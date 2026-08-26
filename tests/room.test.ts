import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  prepareInstallFixture,
  runCli,
  runInstalledCliAt,
  withFixture,
} from "./helpers.ts";

const shellSourceLine =
  '[[ -f "$HOME/maestro/shellrc" ]] && source "$HOME/maestro/shellrc" # maestro';

test("234 install twice preserves a first-edit shell backup and one managed source and registry line", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const originalRc = "# iris autocomplete remains disabled\nexport OWNER_SETTING=kept\n";
    await writeFile(join(fixture.home, ".zshrc"), originalRc);

    const first = await runCli(fixture, ["install"], { PATH: path });
    const second = await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path });

    expect(first.exitCode).toBe(0);
    expect(second.exitCode).toBe(0);
    expect(await readFile(join(fixture.home, ".zshrc.maestro.bak"), "utf8")).toBe(originalRc);
    const rc = await readFile(join(fixture.home, ".zshrc"), "utf8");
    expect(rc.split("\n").filter((line) => line === shellSourceLine)).toHaveLength(1);
    expect(rc).toContain("# iris autocomplete remains disabled");
    expect(rc).toContain("export OWNER_SETTING=kept");
    const registry = (await readFile(join(fixture.home, "maestro", "registry"), "utf8"))
      .trim()
      .split("\n");
    expect(registry).toEqual([await realpath(fixture.repo)]);
  });
});

test("238 install registers each repository exactly once", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const secondRepo = join(fixture.root, "second-repo");
    await mkdir(secondRepo, { recursive: true });

    const firstSecondRepoInstall = await runInstalledCliAt(
      fixture,
      secondRepo,
      ["install"],
      { PATH: path },
    );
    const repeatedSecondRepoInstall = await runInstalledCliAt(
      fixture,
      secondRepo,
      ["install"],
      { PATH: path },
    );

    expect(firstSecondRepoInstall.exitCode).toBe(0);
    expect(repeatedSecondRepoInstall.exitCode).toBe(0);
    const registry = (await readFile(join(fixture.home, "maestro", "registry"), "utf8"))
      .trim()
      .split("\n");
    expect(registry).toEqual([await realpath(fixture.repo), await realpath(secondRepo)]);
  });
});

test.skipIf(process.env.HERDR_ENV !== "1")(
  "235 install scaffolds the room and hm creates then focuses one maestro workspace",
  async () => {
    await withFixture(async (fixture) => {
      const { path } = await prepareInstallFixture(fixture);
      const installed = await runCli(fixture, ["install"], { PATH: path });
      expect(installed.exitCode).toBe(0);

      const room = join(fixture.home, "maestro");
      for (const name of ["IDENTITY.md", "OWNER.md", "AGENTS.md", "CLAUDE.md", "lane.md", "shellrc"]) {
        expect(await Bun.file(join(room, name)).exists()).toBe(true);
      }
      const agents = await readFile(join(room, "AGENTS.md"), "utf8");
      const claude = await readFile(join(room, "CLAUDE.md"), "utf8");
      const lane = await readFile(join(room, "lane.md"), "utf8");
      expect(agents).toContain("lane.md");
      expect(claude).toContain("lane.md");
      expect(agents.split("\n").filter(Boolean).length).toBeLessThanOrEqual(6);
      expect(claude.split("\n").filter(Boolean).length).toBeLessThanOrEqual(6);
      expect(lane).toContain("herdr pane split");
      expect(lane).toContain("herdr agent start");
      expect(lane).toContain("events.wait");
      expect(lane).toContain("maestro handback file");

      const fakeBin = join(fixture.root, "fake-bin");
      const state = join(fixture.root, "herdr-state");
      const log = join(fixture.root, "herdr.log");
      await mkdir(fakeBin, { recursive: true });
      await writeFile(
        join(fakeBin, "herdr"),
        `#!/bin/sh
printf '%s\\n' "$*" >> "$HERDR_LOG"
case "$1 $2" in
  "workspace list")
    if [ -f "$HERDR_STATE" ]; then
      printf '%s\\n' '{"id":"test","result":{"type":"workspace_list","workspaces":[{"label":"maestro","workspace_id":"w9"}]}}'
    else
      printf '%s\\n' '{"id":"test","result":{"type":"workspace_list","workspaces":[]}}'
    fi
    ;;
  "workspace create")
    : > "$HERDR_STATE"
    printf '%s\\n' '{"id":"test","result":{"workspace":{"label":"maestro","workspace_id":"w9"}}}'
    ;;
  "workspace focus")
    printf '%s\\n' '{"id":"test","result":{"workspace_id":"w9"}}'
    ;;
esac
`,
      );
      await chmod(join(fakeBin, "herdr"), 0o755);
      await writeFile(join(fakeBin, "maestro"), "#!/bin/sh\nexit 0\n");
      await chmod(join(fakeBin, "maestro"), 0o755);

      const shell = Bun.spawn(
        ["/bin/zsh", "-f", "-c", 'source "$HOME/maestro/shellrc"; eval hm; eval hm'],
        {
          cwd: fixture.repo,
          env: {
            ...process.env,
            HERDR_ENV: "1",
            HERDR_LOG: log,
            HERDR_STATE: state,
            HOME: fixture.home,
            PATH: `${fakeBin}:${path}`,
          },
          stdout: "pipe",
          stderr: "pipe",
        },
      );
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(shell.stdout).text(),
        new Response(shell.stderr).text(),
        shell.exited,
      ]);
      expect({ stdout, stderr, exitCode }).toEqual({ stdout: "", stderr: "", exitCode: 0 });
      const commands = (await readFile(log, "utf8")).trim().split("\n");
      expect(commands.filter((line) => line.startsWith("workspace create "))).toEqual([
        `workspace create --cwd ${room} --label maestro --focus`,
      ]);
      expect(commands.filter((line) => line.startsWith("workspace focus "))).toEqual([
        "workspace focus w9",
      ]);
    });
  },
);
