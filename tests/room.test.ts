import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { chmod, mkdir, readFile, readdir, realpath, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runInstalledCliAt,
  runTool,
  withFixture,
  type Fixture,
} from "./helpers.ts";

const shellSourceLine =
  '[[ -f "$HOME/maestro/shellrc" ]] && source "$HOME/maestro/shellrc" # maestro';

async function storeSnapshot(repo: string): Promise<Array<[string, number, string]>> {
  const directory = join(repo, ".maestro");
  const names = (await readdir(directory))
    .filter((name) => name === "maestro.db" || name === "maestro.db-wal")
    .sort();
  return Promise.all(
    names.map(async (name) => {
      const path = join(directory, name);
      return [name, (await stat(path)).mtimeMs, (await readFile(path)).toString("base64")];
    }),
  );
}

async function addBriefWork(
  fixture: Fixture,
  repo: string,
  path: string,
  title: string,
  extra: string[] = [],
): Promise<string> {
  return idFrom(
    await runInstalledCliAt(
      fixture,
      repo,
      ["work", "add", title, "--atomic-reason", "brief fixture", ...extra],
      { PATH: path },
    ),
  );
}

async function addRepeatedFailure(
  fixture: Fixture,
  repo: string,
  path: string,
  title: string,
  holder: string,
): Promise<string> {
  const work = await addBriefWork(fixture, repo, path, title);
  const environment = {
    MAESTRO_SESSION_ID: holder,
    MAESTRO_SESSION_PID: String(process.pid),
    PATH: path,
  };
  expect(
    (await runInstalledCliAt(fixture, repo, ["work", "start", work], environment)).exitCode,
  ).toBe(0);
  for (const note of ["failed: first", "failed: second", "failed: third"]) {
    expect(
      (await runInstalledCliAt(fixture, repo, ["work", "note", work, note], environment))
        .exitCode,
    ).toBe(0);
  }
  return work;
}

function openRepoDatabase(repo: string): Database {
  return new Database(join(repo, ".maestro", "maestro.db"));
}

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

test("267 reinstall preserves OWNER.md while refreshing generated room files", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const generatedNames = ["IDENTITY.md", "AGENTS.md", "CLAUDE.md", "lane.md", "shellrc"];
    const generated = new Map(
      await Promise.all(
        generatedNames.map(async (name) => [name, await readFile(join(room, name), "utf8")] as const),
      ),
    );
    const ownerEdit = "# OWNER\n\nOwner-authored content survives installs.\n";
    await writeFile(join(room, "OWNER.md"), ownerEdit);
    for (const name of generatedNames) {
      await writeFile(join(room, name), `stale ${name}\n`);
    }

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(await readFile(join(room, "OWNER.md"), "utf8")).toBe(ownerEdit);
    for (const name of generatedNames) {
      expect(await readFile(join(room, name), "utf8")).toBe(generated.get(name) as string);
    }
  });
});

test("268 first install seeds a neutral OWNER.md", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const owner = await readFile(join(fixture.home, "maestro", "OWNER.md"), "utf8");

    expect(owner).toContain("Working environment, project locations, tools, and recurring constraints.");
    expect(owner).toContain("Communication style, collaboration preferences, and standing boundaries.");
    expect(owner).not.toMatch(/~\/|\/Users\/|macOS|Windows|Linux|Vietnamese|English/);
  });
});

test("269 install wires both room harnesses without overwriting OWNER.md", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const ownerEdit = "# OWNER\n\nRoom harness wiring preserves this owner edit.\n";
    await writeFile(join(room, "OWNER.md"), ownerEdit);

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(await readFile(join(room, "OWNER.md"), "utf8")).toBe(ownerEdit);
    for (const [harness, configName] of [
      ["claude", "settings.json"],
      ["codex", "hooks.json"],
    ] as const) {
      const hookPath = join(room, `.${harness}`, "hooks", "maestro-record.ts");
      const config = JSON.parse(
        await readFile(join(room, `.${harness}`, configName), "utf8"),
      ) as { hooks: Record<string, Array<{ hooks: Array<{ command: string }> }>> };
      expect(await readFile(hookPath, "utf8")).toContain(`--harness\", \"${harness}`);
      expect(config.hooks.SessionStart?.at(-1)?.hooks[0]?.command).toBe(
        `bun .${harness}/hooks/maestro-record.ts`,
      );
      expect(config.hooks.UserPromptSubmit?.at(-1)?.hooks[0]?.command).toBe(
        `bun .${harness}/hooks/maestro-record.ts`,
      );
      expect(config.hooks.PostToolUse).toBeUndefined();
    }
  });
});

test("303 reinstall repairs private room and machine-record permissions", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const privateDirectories = [room, join(room, ".maestro")];
    const privateFiles = [
      join(room, "OWNER.md"),
      join(room, "registry"),
      join(room, ".maestro", "maestro.db"),
      join(fixture.home, ".maestro", "source.json"),
    ];
    for (const directory of privateDirectories) await chmod(directory, 0o755);
    for (const file of privateFiles) await chmod(file, 0o644);

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    for (const directory of privateDirectories) {
      expect((await stat(directory)).mode & 0o777).toBe(0o700);
    }
    for (const file of privateFiles) {
      expect((await stat(file)).mode & 0o777).toBe(0o600);
    }
    for (const suffix of ["-wal", "-shm"]) {
      const sidecar = join(room, ".maestro", `maestro.db${suffix}`);
      if (existsSync(sidecar)) expect((await stat(sidecar)).mode & 0o777).toBe(0o600);
    }
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
      expect(lane).toContain("herdr agent wait <name> --until working --timeout 60000");
      expect(lane).toContain("`herdr agent wait <name>` with no `--until`");
      expect(lane).not.toContain("--until done");
      expect(lane).toContain("re-arm");
      expect(lane).toContain("holder shown by `maestro status --live` is the authority");
      expect(lane).toContain("herdr pane close <pane-id>");
      expect(lane).toContain("herdr tab close <tab-id>");
      expect(lane).toContain("stays only when the same lane takes the next dispatch");
      expect(lane).not.toContain("events.wait");
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

test("247 room harness files give agents the pane-lane contract without a lane skill", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const agents = await readFile(join(room, "AGENTS.md"), "utf8");
    const claude = await readFile(join(room, "CLAUDE.md"), "utf8");

    expect(agents).toBe(claude);
    expect(agents).toContain("Lanes are Herdr panes, never sub-agents.");
    expect(agents).toContain("read `lane.md`");
    expect(agents).not.toContain("SKILL.md");
    expect((await readdir(join(room, "skills"))).sort()).toEqual([
      "maestro-bundle",
      "maestro-design",
      "maestro-verify",
      "maestro-work",
    ]);
  });
});

test("248 project harness files do not give agents room-only lane instructions", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const roomAgents = await readFile(join(fixture.home, "maestro", "AGENTS.md"), "utf8");
    expect(roomAgents).toContain("Lanes are Herdr panes, never sub-agents.");

    for (const name of ["AGENTS.md", "CLAUDE.md"]) {
      const projectInstructions = await readFile(join(fixture.repo, name), "utf8");
      expect(projectInstructions).not.toContain("Lanes are Herdr panes");
      expect(projectInstructions).not.toContain("lane.md");
      expect(projectInstructions).not.toContain("herdr pane");
      expect(projectInstructions).not.toContain("herdr agent");
    }
  });
});

test("250 installed lane guidance names the runnable Herdr wait command", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const lane = await readFile(join(fixture.home, "maestro", "lane.md"), "utf8");

    expect(lane).toContain(
      "`herdr agent wait <name>` with no `--until` as a background command",
    );
    expect(lane).not.toContain("herdr events");
    expect(lane).not.toContain("events.wait");
  });
});

test("265 every installed lane Maestro command parses against the real CLI", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lane = await readFile(join(fixture.home, "maestro", "lane.md"), "utf8");
    const commands = [
      ...new Set([...lane.matchAll(/`(maestro [^`\n]+)`/g)].map((match) => match[1] as string)),
    ];

    expect(lane).toContain(
      "herdr tab create --workspace <workspace-id> --cwd <repo> --label lanes --no-focus",
    );
    expect(lane).not.toContain("herdr pane split --current");
    expect(lane).not.toMatch(/\.{3}|…/);
    expect(lane).toContain("herdr pane process-info --pane <pane-id>");
    expect(lane).toContain("the session whose pid matches in `maestro status --live`");
    expect(lane).not.toContain("Reply with the single word");
    expect(lane).not.toContain("ask the started lane");
    expect(lane).toContain(
      "Never send a warm-up prompt just to learn the id.",
    );
    expect(lane).toContain("Never treat the pane id as session identity.");
    expect(commands).toEqual([
      'maestro work add "<title>" --atomic-reason "<why>"',
      "maestro status --live",
      'maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id> --target-session <session-id>',
      "maestro dispatch show <dispatch-id>",
      "maestro dispatch list <work-id>",
      "maestro dispatch accept <dispatch-id>",
      'maestro handback file <dispatch-id> --status DONE --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"',
      "maestro brief",
    ]);

    const replacements = new Map([
      ["<title>", "lane drift fixture"],
      ["<why>", "parser proof"],
      ["<observable outcome>", "commands parse"],
      ["<paths or responsibility>", "fixture store"],
      ["<explicit non-goals>", "product source"],
      ["<no-write or write-bounded paths>", "no-write"],
      ["<done or blocked boundary>", "handback filed"],
      ["<falsifier>", "real CLI accepted command"],
      ["<pane-id>", "w1:pZ"],
      ["<session-id>", "test-session"],
      ["<current belief>", "commands parse"],
    ]);
    const argumentsFor = (command: string): string[] => {
      let rendered = command;
      for (const [placeholder, value] of replacements) {
        rendered = rendered.replaceAll(placeholder, value);
      }
      const tokens = rendered.match(/"[^"]*"|\S+/g) ?? [];
      return tokens.slice(1).map((token) =>
        token.startsWith('"') && token.endsWith('"') ? token.slice(1, -1) : token
      );
    };

    for (const command of commands) {
      const parsed = await runInstalledCliAt(
        fixture,
        fixture.repo,
        argumentsFor(command),
        { PATH: path },
      );
      expect(parsed.exitCode, `${command}\n${parsed.stderr}`).toBe(0);
      if (command.startsWith("maestro work add ")) {
        replacements.set("<work-id>", idFrom(parsed));
      }
      if (command.startsWith("maestro dispatch open ")) {
        replacements.set("<dispatch-id>", parsed.stdout.match(/^(x\d+)/)?.[1] as string);
      }
    }
  });
});

test("283 status marks exactly the caller session in bare, live, and JSON output", async () => {
  await withFixture(async (fixture) => {
    const caller = { MAESTRO_SESSION_ID: "caller", MAESTRO_SESSION_PID: String(process.pid) };
    const peer = { MAESTRO_SESSION_ID: "peer", MAESTRO_SESSION_PID: String(process.pid) };
    for (const environment of [caller, peer]) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
            environment,
          )
        ).exitCode,
      ).toBe(0);
    }

    for (const args of [["status"], ["status", "--live"]]) {
      const status = await runCli(fixture, args, caller);
      const marked = status.stdout.split("\n").filter((line) => line.endsWith(" (this session)"));
      expect(status.exitCode).toBe(0);
      expect(marked).toHaveLength(1);
      expect(marked[0]).toStartWith("caller ");
    }

    const json = await runCli(fixture, ["status", "--live", "--json"], caller);
    expect(json.exitCode).toBe(0);
    expect((JSON.parse(json.stdout) as { data: { currentSession: string } }).data.currentSession)
      .toBe("caller");
  });
});

test("275 the installed room overlays Herdr agent status on work-scoped lane rows once", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const shellrc = await readFile(join(fixture.home, "maestro", "shellrc"), "utf8");
    expect(shellrc).toContain("function maestro_lanes()");
    expect(shellrc).toContain("printf '%s\\n' \"$lanes\"");

    const fakeBin = join(fixture.root, "fake-bin-lanes");
    const maestroLog = join(fixture.root, "maestro-lanes.log");
    const herdrLog = join(fixture.root, "herdr-lanes.log");
    await mkdir(fakeBin, { recursive: true });
    await writeFile(
      join(fakeBin, "maestro"),
      `#!/bin/sh
printf '%s\n' "$*" >> "$MAESTRO_LOG"
printf '%s\n' 'council: sealed (0/2 returned)' '' 'lane w2:p2 | x11 | decision | dispatch=open | work=active | holder=live' 'lane w2:p3 | x12 | delivery | dispatch=open | work=active | holder=live'
`,
    );
    await chmod(join(fakeBin, "maestro"), 0o755);
    await writeFile(
      join(fakeBin, "herdr"),
      `#!/bin/sh
printf '%s\n' "$*" >> "$HERDR_LOG"
printf '%s\n' '{"result":{"agents":[{"pane_id":"w2:p2","agent_status":"working"},{"pane_id":"w2:p3","agent_status":"blocked"}]}}'
`,
    );
    await chmod(join(fakeBin, "herdr"), 0o755);

    const shell = Bun.spawn(
      ["/bin/zsh", "-f", "-c", 'source "$HOME/maestro/shellrc"; maestro_lanes w81'],
      {
        cwd: fixture.repo,
        env: {
          ...process.env,
          HERDR_LOG: herdrLog,
          HOME: fixture.home,
          MAESTRO_LOG: maestroLog,
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

    expect(exitCode, stderr).toBe(0);
    expect(stderr).toBe("");
    expect(stdout).toBe(
      "council: sealed (0/2 returned)\n\n" +
        "lane w2:p2 | x11 | decision | dispatch=open | work=active | holder=live | agent=working\n" +
        "lane w2:p3 | x12 | delivery | dispatch=open | work=active | holder=live | agent=blocked\n",
    );
    expect(await readFile(maestroLog, "utf8")).toBe("dispatch list w81\n");
    expect(await readFile(herdrLog, "utf8")).toBe("agent list\n");
  });
});

test("276 brief omits lane rows while pane-bound dispatches are open", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const work = await addBriefWork(fixture, fixture.repo, path, "ordinary lane progress");
    const dispatches: string[] = [];
    for (const [pane, holder] of [["w1:pA", "lane-a"], ["w1:pB", "lane-b"]] as const) {
      const opened = await runInstalledCliAt(
        fixture,
        fixture.repo,
        [
          "dispatch",
          "open",
          work,
          "--objective",
          "run an ordinary lane",
          "--owned-scope",
          "fixture",
          "--excluded-scope",
          "product source",
          "--mutation",
          "no-write",
          "--stop-condition",
          "handback filed",
          "--lane",
          "delivery",
          "--evidence-required",
          "source: fixture",
          "--pane",
          pane,
        ],
        { PATH: path },
      );
      expect(opened.exitCode).toBe(0);
      const dispatch = opened.stdout.match(/^(x\d+)/)?.[1] as string;
      dispatches.push(dispatch);
      const environment = {
        MAESTRO_SESSION_ID: holder,
        MAESTRO_SESSION_PID: String(process.pid),
        PATH: path,
      };
      expect(
        (
          await runInstalledCliAt(
            fixture,
            fixture.repo,
            ["hook", "record", "--event", "SessionStart"],
            environment,
          )
        ).exitCode,
      ).toBe(0);
      expect(
        (
          await runInstalledCliAt(
            fixture,
            fixture.repo,
            ["dispatch", "accept", dispatch],
            environment,
          )
        ).exitCode,
      ).toBe(0);
    }

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toBe("All registered projects are running normally.\n");
    expect(brief.stdout).not.toContain("lane w1:");
    expect(brief.stdout).not.toContain("ordinary lane progress");
    for (const dispatch of dispatches) expect(brief.stdout).not.toContain(dispatch);
  });
});

test("237 brief says every registered repository is running normally in one line", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const secondRepo = join(fixture.root, "normal-repo");
    await mkdir(secondRepo, { recursive: true });
    await runInstalledCliAt(fixture, secondRepo, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary alpha progress");
    await addBriefWork(fixture, secondRepo, path, "ordinary beta progress");

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );

    expect(brief.exitCode).toBe(0);
    expect(brief.stderr).toBe("");
    expect(brief.stdout).toBe("All registered projects are running normally.\n");
    expect(brief.stdout).not.toContain("ordinary alpha progress");
    expect(brief.stdout).not.toContain("ordinary beta progress");
    expect(brief.stdout).not.toContain(await realpath(fixture.repo));
    expect(brief.stdout).not.toContain(await realpath(secondRepo));
  });
});

test("239 brief reports detector findings from two repositories without writing either store", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const secondRepo = join(fixture.root, "open-repo");
    await mkdir(secondRepo, { recursive: true });
    await runInstalledCliAt(fixture, secondRepo, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary alpha progress");
    await addBriefWork(fixture, secondRepo, path, "ordinary beta progress");
    const alpha = await addRepeatedFailure(
      fixture,
      fixture.repo,
      path,
      "alpha repeatedly failing",
      "alpha-holder",
    );
    const beta = await addRepeatedFailure(
      fixture,
      secondRepo,
      path,
      "beta repeatedly failing",
      "beta-holder",
    );
    const before = [await storeSnapshot(fixture.repo), await storeSnapshot(secondRepo)];

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );

    expect(brief.exitCode).toBe(0);
    expect(brief.stderr).toBe("");
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention REPEATED_FAILURE ${alpha}`,
    );
    expect(brief.stdout).toContain(
      `${await realpath(secondRepo)}: attention REPEATED_FAILURE ${beta}`,
    );
    expect(brief.stdout).not.toContain("ordinary alpha progress");
    expect(brief.stdout).not.toContain("ordinary beta progress");
    expect([await storeSnapshot(fixture.repo), await storeSnapshot(secondRepo)]).toEqual(before);
  });
});

test("240 brief names a deleted registered repository and continues", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary live progress");
    const deletedRepo = join(fixture.root, "deleted-repo");
    await mkdir(deletedRepo, { recursive: true });
    await runInstalledCliAt(fixture, deletedRepo, ["install"], { PATH: path });
    const deletedPath = await realpath(deletedRepo);
    await rm(deletedRepo, { recursive: true, force: true });

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );

    expect(brief.exitCode).toBe(0);
    expect(brief.stderr).toBe("");
    expect(brief.stdout).toContain(`Missing repository: ${deletedPath}`);
    expect(brief.stdout).not.toContain("ordinary live progress");
  });
});

test("251 brief reports DECISION_STALE and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary decision progress");
    const work = await addBriefWork(fixture, fixture.repo, path, "owner decision needed");
    const decision = idFrom(
      await runInstalledCliAt(
        fixture,
        fixture.repo,
        ["decision", "draft", "choose owner boundary", "--work", work],
        { PATH: path },
      ),
    );
    const database = openRepoDatabase(fixture.repo);
    database.query("UPDATE decisions SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    database.close();

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention DECISION_STALE ${decision}`,
    );
    expect(brief.stdout).not.toContain("ordinary decision progress");
  });
});

test("252 brief reports STALLED_LEASE and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary stalled progress");
    const work = await addBriefWork(fixture, fixture.repo, path, "stalled lane");
    const environment = {
      MAESTRO_SESSION_ID: "stalled-holder",
      MAESTRO_SESSION_PID: String(process.pid),
      PATH: path,
    };
    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["work", "start", work], environment))
        .exitCode,
    ).toBe(0);
    const database = openRepoDatabase(fixture.repo);
    database.query("UPDATE sessions SET pid = 1, anchor = 'pid', last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 31 * 60_000).toISOString(), "stalled-holder");
    database.close();

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention STALLED_LEASE ${work}`,
    );
    expect(brief.stdout).not.toContain("ordinary stalled progress");
  });
});

test("253 brief reports REPEATED_FAILURE and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary retry progress");
    const work = await addRepeatedFailure(
      fixture,
      fixture.repo,
      path,
      "repeatedly failing lane",
      "failure-holder",
    );

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention REPEATED_FAILURE ${work}`,
    );
    expect(brief.stdout).not.toContain("ordinary retry progress");
  });
});

test("254 brief reports DISPATCH_UNRETURNED and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary dispatch progress");
    const work = await addBriefWork(fixture, fixture.repo, path, "unreturned lane");
    const opened = await runInstalledCliAt(
      fixture,
      fixture.repo,
      [
        "dispatch",
        "open",
        work,
        "--objective",
        "return the result",
        "--owned-scope",
        "scratch",
        "--excluded-scope",
        "product source",
        "--mutation",
        "no-write",
        "--stop-condition",
        "handback filed",
        "--lane",
        "delivery",
        "--evidence-required",
        "source",
        "--pane",
        "w1:pZ",
      ],
      { PATH: path },
    );
    expect(opened.exitCode).toBe(0);
    const dispatch = opened.stdout.match(/^(x\d+)/)?.[1] as string;
    const database = openRepoDatabase(fixture.repo);
    database.query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 3 * 60 * 60_000).toISOString(), dispatch);
    database.close();

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention DISPATCH_UNRETURNED ${dispatch}`,
    );
    expect(brief.stdout).not.toContain("ordinary dispatch progress");
  });
});

test("255 brief reports SCOPE_COLLISION and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary collision progress");
    const parent = await addBriefWork(fixture, fixture.repo, path, "shared mutation scope");
    const first = await addBriefWork(
      fixture,
      fixture.repo,
      path,
      "first colliding lane",
      ["--parent", parent],
    );
    const second = await addBriefWork(
      fixture,
      fixture.repo,
      path,
      "second colliding lane",
      ["--parent", parent],
    );
    for (const [work, holder] of [[first, "collision-a"], [second, "collision-b"]] as const) {
      expect(
        (
          await runInstalledCliAt(fixture, fixture.repo, ["work", "start", work], {
            MAESTRO_SESSION_ID: holder,
            MAESTRO_SESSION_PID: String(process.pid),
            PATH: path,
          })
        ).exitCode,
      ).toBe(0);
    }

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention SCOPE_COLLISION ${first},${second}`,
    );
    expect(brief.stdout).not.toContain("ordinary collision progress");
  });
});

test("241 install moves the four method skills into the room and links only those into Claude", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const unrelatedAgentSkill = join(fixture.home, ".agents", "skills", "unrelated", "SKILL.md");
    const excludedClaudeSkill = join(
      fixture.home,
      ".claude",
      "skills",
      "maestro-lifecycle-test",
      "SKILL.md",
    );
    await mkdir(join(unrelatedAgentSkill, ".."), { recursive: true });
    await mkdir(join(excludedClaudeSkill, ".."), { recursive: true });
    await writeFile(unrelatedAgentSkill, "# unrelated agent skill\n");
    await writeFile(excludedClaudeSkill, "# excluded Claude skill\n");

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    for (const name of ["maestro-bundle", "maestro-design", "maestro-work", "maestro-verify"]) {
      const roomSkill = join(fixture.home, "maestro", "skills", name, "SKILL.md");
      expect(await readFile(roomSkill, "utf8")).toMatch(
        /<!-- maestro-skill-version: [0-9a-f]{40} -->/,
      );
      expect(await realpath(join(fixture.home, ".claude", "skills", name))).toBe(
        await realpath(join(fixture.home, "maestro", "skills", name)),
      );
      expect(await Bun.file(join(fixture.home, ".agents", "skills", name)).exists()).toBe(false);
    }
    expect(await readFile(unrelatedAgentSkill, "utf8")).toBe("# unrelated agent skill\n");
    expect(await readFile(excludedClaudeSkill, "utf8")).toBe("# excluded Claude skill\n");
    expect(
      await Bun.file(
        join(fixture.home, "maestro", "skills", "maestro-verify", "references", "audit.md"),
      ).exists(),
    ).toBe(true);
  });
});

test("242 owner preferences are room decisions whose reversals supersede the prior choice", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const room = join(fixture.home, "maestro");
    const owner = await readFile(join(room, "OWNER.md"), "utf8");
    expect(owner).toContain("maestro decision draft");
    expect(owner).toContain("--supersedes");

    const first = await runInstalledCliAt(
      fixture,
      room,
      [
        "decision",
        "draft",
        "Prefer terse project briefs",
        "--rationale",
        "the owner asked for less noise",
      ],
      { PATH: path },
    );
    const firstId = idFrom(first);
    expect((await runInstalledCliAt(fixture, room, ["decision", "lock", firstId], { PATH: path })).exitCode)
      .toBe(0);
    const reversal = await runInstalledCliAt(
      fixture,
      room,
      [
        "decision",
        "draft",
        "Prefer detailed project briefs",
        "--rationale",
        "the owner reversed the earlier preference",
        "--supersedes",
        firstId,
      ],
      { PATH: path },
    );
    const reversalId = idFrom(reversal);
    expect(
      (await runInstalledCliAt(fixture, room, ["decision", "lock", reversalId], { PATH: path }))
        .exitCode,
    ).toBe(0);

    const listed = await runInstalledCliAt(
      fixture,
      room,
      ["decision", "list", "--json"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    const envelope = JSON.parse(listed.stdout) as {
      data: {
        decisions: Array<{
          id: string;
          state: string;
          supersededById: string | null;
          supersedesId: string | null;
        }>;
      };
    };
    expect(envelope.data.decisions).toContainEqual(
      expect.objectContaining({ id: firstId, state: "superseded", supersededById: reversalId }),
    );
    expect(envelope.data.decisions).toContainEqual(
      expect.objectContaining({ id: reversalId, state: "locked", supersedesId: firstId }),
    );
  });
});

test("243 install initializes the room store without turning the room into a git repository", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    const room = join(fixture.home, "maestro");

    expect(installed.exitCode).toBe(0);
    expect((await runTool(["git", "rev-parse", "--show-toplevel"], room)).exitCode).not.toBe(0);
    expect(await Bun.file(join(room, ".git")).exists()).toBe(false);
    expect(await Bun.file(join(room, ".maestro", "maestro.db")).exists()).toBe(true);
    expect(await Bun.file(join(fixture.home, ".maestro", "maestro.db")).exists()).toBe(false);

    const added = await runInstalledCliAt(
      fixture,
      room,
      ["work", "add", "unassigned owner idea", "--atomic-reason", "room record"],
      { PATH: path },
    );
    expect(added.exitCode).toBe(0);
    const shown = await runInstalledCliAt(
      fixture,
      room,
      ["work", "show", idFrom(added)],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(shown.stdout).toContain("unassigned owner idea");
  });
});

test("244 install leaves existing Irina instructions byte-identical", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const agents = join(fixture.home, "Code", "irina", "AGENTS.md");
    const original = "# Irina — Chief of Staff\n\nOriginal instructions stay byte-for-byte below.\n";
    await mkdir(join(agents, ".."), { recursive: true });
    await writeFile(agents, original);

    const first = await runCli(fixture, ["install"], { PATH: path });
    const second = await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path });

    expect(await readFile(agents, "utf8")).toBe(original);
    expect(first.stdout).not.toContain("retired:");
    expect(second.stdout).not.toContain("retired:");
  });
});

test.skipIf(process.env.HERDR_ENV !== "1")(
  "236 hm prints the read-only brief and returns without starting an agent",
  async () => {
    await withFixture(async (fixture) => {
      const { path } = await prepareInstallFixture(fixture);
      await runCli(fixture, ["install"], { PATH: path });
      const fakeBin = join(fixture.root, "fake-bin-brief");
      const herdrLog = join(fixture.root, "herdr-brief.log");
      const maestroLog = join(fixture.root, "maestro-brief.log");
      await mkdir(fakeBin, { recursive: true });
      await writeFile(
        join(fakeBin, "herdr"),
        `#!/bin/sh
printf '%s\\n' "$*" >> "$HERDR_LOG"
printf '%s\\n' '{"id":"test","result":{"type":"workspace_list","workspaces":[{"label":"maestro","workspace_id":"w9"}]}}'
`,
      );
      await chmod(join(fakeBin, "herdr"), 0o755);
      await writeFile(
        join(fakeBin, "maestro"),
        `#!/bin/sh
printf '%s read-only=%s cwd=%s\\n' "$*" "$MAESTRO_READ_ONLY" "$PWD" >> "$MAESTRO_LOG"
printf '%s\\n' 'owner brief'
`,
      );
      await chmod(join(fakeBin, "maestro"), 0o755);

      const shell = Bun.spawn(
        [
          "/bin/zsh",
          "-f",
          "-c",
          'source "$HOME/maestro/shellrc"; eval hm; printf "shell-returned\\n"',
        ],
        {
          cwd: fixture.repo,
          env: {
            ...process.env,
            HERDR_ENV: "1",
            HERDR_LOG: herdrLog,
            HOME: fixture.home,
            MAESTRO_LOG: maestroLog,
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

      expect(exitCode).toBe(0);
      expect(stderr).toBe("");
      expect(stdout).toBe("owner brief\nshell-returned\n");
      expect(await readFile(maestroLog, "utf8")).toBe(
        `brief read-only=1 cwd=${join(fixture.home, "maestro")}\n`,
      );
      const herdrCommands = await readFile(herdrLog, "utf8");
      expect(herdrCommands).toContain("workspace list");
      expect(herdrCommands).toContain("workspace focus w9");
      expect(herdrCommands).not.toContain("agent");
    });
  },
);
