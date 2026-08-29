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
  const entries = await Promise.all(
    names.map(async (name): Promise<[string, number, string]> => {
      const path = join(directory, name);
      return [name, (await stat(path)).mtimeMs, (await readFile(path)).toString("base64")];
    }),
  );
  // SQLite creates an empty -wal sidecar on open (Linux); only a non-empty one is a write.
  return entries.filter(([name, , content]) => name === "maestro.db" || content !== "");
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

function roomMetaRows(room: string): Array<{ key: string; value: string }> {
  const database = openRepoDatabase(room);
  try {
    return database
      .query<{ key: string; value: string }, []>("SELECT key, value FROM meta ORDER BY key")
      .all();
  } finally {
    database.close();
  }
}

test("234 install twice preserves a first-edit shell backup and one managed source and registry line", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const originalRc = "# iris autocomplete remains disabled\nexport OWNER_SETTING=kept\n";
    await writeFile(join(fixture.home, ".zshrc"), originalRc);

    const shell = { PATH: path, SHELL: "/bin/zsh" };
    const first = await runCli(fixture, ["install"], shell);
    const second = await runInstalledCliAt(fixture, fixture.repo, ["install"], shell);

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
    const generatedNames = [
      "IDENTITY.md",
      "AGENTS.md",
      "CLAUDE.md",
      "lane.md",
      "lead.md",
      "observer.md",
      "observer-watch.sh",
      "shellrc",
      ".claude/settings.json",
    ];
    const firstInstall = new Map(
      await Promise.all(
        generatedNames.map(async (name) => [name, await readFile(join(room, name), "utf8")] as const),
      ),
    );
    const ownerEdit = "# OWNER\n\nOwner-authored content survives installs.\n";
    await writeFile(join(room, "OWNER.md"), ownerEdit);
    for (const name of generatedNames) {
      await writeFile(
        join(room, name),
        name === ".claude/settings.json" ? "{}\n" : `stale ${name}\n`,
      );
    }

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(await readFile(join(room, "OWNER.md"), "utf8")).toBe(ownerEdit);
    for (const name of generatedNames) {
      expect(await readFile(join(room, name), "utf8")).toBe(firstInstall.get(name) ?? "");
      expect(await readFile(join(room, name), "utf8")).not.toContain("stale ");
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
      join(room, ".claude", "settings.json"),
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
  "235 [lint] install scaffolds the room and hm creates then focuses one maestro workspace",
  async () => {
    // Shell-adapter lint: proves generated command wiring, not a real Herdr create/focus journey.
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
      expect(lane).toContain(
        "herdr agent wait peer-<dispatch id> --until working --timeout 60000",
      );
      expect(lane).toContain("`herdr agent wait peer-<dispatch id>` with no `--until`");
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
      const agentState = join(fixture.root, "herdr-agent-state");
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
  "pane list")
    printf '%s\\n' '{"id":"test","result":{"type":"pane_list","panes":[{"pane_id":"w9:p1","workspace_id":"w9"}]}}'
    ;;
  "agent list")
    if [ -f "$HERDR_AGENT_STATE" ]; then
      printf '%s\\n' '{"id":"test","result":{"type":"agent_list","agents":[{"name":"supervisor","pane_id":"w9:p1","workspace_id":"w9"}]}}'
    else
      printf '%s\\n' '{"id":"test","result":{"type":"agent_list","agents":[]}}'
    fi
    ;;
  "agent start")
    : > "$HERDR_AGENT_STATE"
    printf '%s\\n' '{"id":"test","result":{"name":"supervisor","pane_id":"w9:p1"}}'
    ;;
  "agent focus")
    printf '%s\\n' '{"id":"test","result":{"name":"supervisor","pane_id":"w9:p1"}}'
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
            HERDR_AGENT_STATE: agentState,
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
      expect(commands.filter((line) => line.startsWith("agent start "))).toEqual([
        "agent start supervisor --kind claude --pane w9:p1 -- --autocompact 250000",
      ]);
      expect(commands.filter((line) => line.startsWith("agent focus "))).toEqual([
        "agent focus supervisor",
      ]);
    });
  },
);

test("247 [lint] room harness files give agents the pane-lane contract without a lane skill", async () => {
  // Packaging lint: proves room instruction files contain the lane contract, not that a harness loads it.
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

test("248 [lint] project harness files do not give agents room-only lane instructions", async () => {
  // Scope lint: proves project mirrors omit room-only text, not that parent instructions cannot inject it.
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

test("250 [lint] installed lane guidance names the runnable Herdr wait command", async () => {
  // Documentation lint: proves the installed command text, not that the current Herdr parser accepts it.
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const lane = await readFile(join(fixture.home, "maestro", "lane.md"), "utf8");

    expect(lane).toContain(
      "`herdr agent wait peer-<dispatch id>` with no `--until` as a background command",
    );
    expect(lane).toContain(
      "`BLOCKED`, `DEPENDENCY_REQUEST`, `COUNCIL_REQUEST`, and `REOPEN_REQUEST` also pass `--request \"<retry condition or requested action>\"`.",
    );
    expect(lane).toContain(
      "verify that `claimed by` or `held by` equals the pane's session",
    );
    expect(lane).toContain(
      "runs `maestro dispatch confirm <dispatch-id> --session <session-id>`",
    );
    expect(lane).toContain(
      "runs `maestro dispatch cancel <dispatch-id> --reason wrong-holder` and opens a new dispatch",
    );
    expect(lane).toContain(
      "A delivery lane passes `--candidate <commit or digest>` with its DONE handback.",
    );
    expect(lane).toContain(
      "The wait is a convenience; the handback in the store is the return; a wait that outlives the handback (a lane with a background shell stays `working`) is resolved by reading the store, never by prompting the lane.",
    );
    expect(lane).not.toContain("herdr events");
    expect(lane).not.toContain("events.wait");
  });
});

test("450 [lint] installed lead guidance hands owner intent to the repository Lead", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const lead = await readFile(join(room, "lead.md"), "utf8");
    const shellrc = await readFile(join(room, "shellrc"), "utf8");
    const identity = await readFile(join(room, "IDENTITY.md"), "utf8");
    const agents = await readFile(join(room, "AGENTS.md"), "utf8");

    expect(lead).not.toContain("`MAESTRO_READ_ONLY=1 maestro status --live`");
    expect(lead).toContain("`herdr agent list`");
    expect(lead).toContain(
      '`herdr agent prompt lead-<repo basename> "[from supervisor][intent] <owner words verbatim>"`',
    );
    expect(lead).toContain(
      "`herdr tab create --workspace <workspace-id> --cwd <repo> --label lead-<repo basename> --no-focus`",
    );
    expect(lead).not.toContain(
      "`herdr tab create --workspace <workspace-id> --cwd <repo> --label lead --no-focus`",
    );
    expect(lead).toContain(
      "`herdr agent start lead-<repo basename> --kind <harness OWNER.md names> --pane <pane-id>`",
    );
    // d24: a Claude Lead gets the same window as a Claude lane.
    expect(lead).toContain(
      "a Claude Lead is started with `-- --model <name> --effort <level> --autocompact 250000`",
    );
    expect(lead).toContain(
      '`herdr agent prompt lead-<repo basename> "[from supervisor][intent] <owner words verbatim>. You are the Lead of <repo>; this is owner intent relayed by the room; record it as work and choose your own route (d700); report to <record holder>; read `~/maestro/PROJECT/<repo basename>.md` before your first card, it holds every correction already filed against this project."`',
    );
    expect(lead).toContain(
      "`herdr agent wait lead-<repo basename> --until working --timeout 60000`",
    );
    expect(lead).toContain(
      '`maestro work note <room-work-id> "handed intent to <repo>: <one-line summary>"`',
    );
    expect(lead).toContain(
      'Never run `maestro work add` or any write in the project store, run `maestro dispatch open`, suggest topology in the prompt, or read the pane transcript.',
    );
    expect(lead).toContain(
      '`maestro decision draft "<the choice>" --rationale "<why, options>" --work <id>`, then `herdr agent prompt supervisor "[from lead][ask d<id>] <question>"`',
    );
    expect(lead).toContain("A non-decision question is a work note sent the same way.");
    expect(lead).toContain("The room never runs `herdr agent wait` on a Lead");
    expect(lead).toContain("the store is the truth and the room's next prompt shows it");
    expect(lead).toContain(
      "The room's reply is a prompt and the record (lock or supersede) is what the Lead acts on.",
    );
    expect(lead).not.toContain(
      "The Lead reports back through its own store and the next `maestro brief`.",
    );
    expect(shellrc).toContain(
      'herdr agent start supervisor --kind claude --pane "$root_pane_id" -- --autocompact 250000',
    );
    // d24 fixes the window here but not the model: the kind and the flags after
    // -- are one owner edit point, and model names live in OWNER.md.
    expect(shellrc).toContain(
      "both this kind and the\n    # flags after -- are the owner's to edit together.",
    );
    expect(identity).toContain("read `lead.md`");
    expect(agents).toContain("read `lead.md`");
  });
});

test("516 [lint] room guidance forbids store hand edits and relays fix intent", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const agents = await readFile(join(room, "AGENTS.md"), "utf8");
    const claude = await readFile(join(room, "CLAUDE.md"), "utf8");
    const lead = await readFile(join(room, "lead.md"), "utf8");
    const storeBoundary =
      "The room never edits any store by hand (no sqlite, no file edits under `.maestro`); every store changes only through `maestro` verbs, and a defect in stored data is owner intent for the Lead of the code that wrote it, relayed per `lead.md`.";
    const relayRule =
      "When the owner says fix or do, relay without asking whether to relay; ask the owner a question only for a real fork.";

    expect(agents).toContain(storeBoundary);
    expect(claude).toContain(storeBoundary);
    expect(lead.split("\n").find((line) => line.startsWith("1. "))).toContain(relayRule);
  });
});

test("528 [lint] a Lead reports a closed card back to the room (d22)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lead = await readFile(join(fixture.home, "maestro", "lead.md"), "utf8");
    const slp = await readFile(
      join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
      "utf8",
    );

    // maestro brief prints attention findings only, so a closed card is
    // invisible to the room; without this sentence a Lead can finish relayed
    // intent and the room never learns, which is how three closures went
    // unreported before d22.
    const message =
      'herdr agent prompt supervisor "[from lead][done w<id> re <room record>] <candidate commit; one line on any deviation>"';
    expect(lead).toContain(message);
    expect(slp).toContain(message);
    expect(lead).toContain("one prompt per closed card, after `maestro work done`");
    expect(lead).toContain("never before");
  });
});

test("501 [lint] installed room guidance selects models from the SLP reference (d711)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const lane = await readFile(join(room, "lane.md"), "utf8");
    const lead = await readFile(join(room, "lead.md"), "utf8");
    const owner = await readFile(join(room, "OWNER.md"), "utf8");
    const laneStep = lane.split("5. ")[1]?.split("\n6. ")[0] ?? "";
    const leadStep = lead.split("4. ")[1]?.split("\n5. ")[0] ?? "";

    expect(laneStep).toContain(
      "Pass the chosen model from the Model table in `maestro recipe show slp` and the lane's thinking level from its table to the harness: use `-- --model <name> --effort <level> --autocompact 250000` for Claude or Codex's `--model <name> -c model_reasoning_effort=<level>` flags (verified with `claude --help` and `codex --help`).",
    );
    expect(leadStep).toContain(
      "Pick the Lead's model from the `lead` rung of the Model table in `maestro recipe show slp`;",
    );
    expect(owner).toContain(
      "Which examples should the owner-editable Model table column use for cheap, strong, diverse, and lead? Keep the current names here.",
    );
  });
});

test("484 [lint] installed Lead guidance uses only the opener-set repository role name", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lead = await readFile(join(fixture.home, "maestro", "lead.md"), "utf8");

    expect(lead).toContain("lead-<repo basename>");
    expect(lead).toContain("whose cwd is the repository");
    expect(lead).toContain("Never prompt a pane with any other name.");
    expect(lead).not.toContain(
      "A Lead is live when a session in the tree holds parent work or is simply live with that cwd.",
    );
    expect(lead).not.toContain("find the pane whose cwd is the repository");
  });
});

test("485 [lint] installed lane guidance binds the Peer name before agent start", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lane = await readFile(join(fixture.home, "maestro", "lane.md"), "utf8");
    const opened = lane.indexOf("`maestro dispatch open <work-id>");
    const started = lane.indexOf(
      "`herdr agent start peer-<dispatch id> --kind <kind> --pane <pane-id>`",
    );

    expect(opened).toBeGreaterThanOrEqual(0);
    expect(started).toBeGreaterThan(opened);
    expect(lane).toContain(
      'not my role: <name> holds <dispatch id>; send intent to the Lead',
    );
    expect(lane).toContain("runs no Maestro write verb and files nothing");
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
      "herdr tab create --workspace <workspace-id> --cwd <repo> --label lanes-<owner>-<work id> --no-focus",
    );
    expect(lane).toContain(
      "`<owner>` is the repository basename when a Lead opens the tab (for example, `lanes-maestro-w524`) and the literal `room` when the Supervisor opens lanes for room-store work (for example, `lanes-room-w522`), because the room directory is also named `maestro` and its basename cannot distinguish room-store work.",
    );
    expect(lane).not.toContain(
      "herdr tab create --workspace <workspace-id> --cwd <repo> --label lanes-<work id> --no-focus",
    );
    expect(lane).toContain("once the `lanes-<owner>-<work id>` tab is empty");
    expect(lane).not.toContain("once the lanes tab is empty");
    expect(lane).not.toContain("herdr pane split --current");
    expect(lane).not.toMatch(/\.{3}|…/);
    expect(lane).toContain("herdr pane process-info --pane <pane-id>");
    expect(lane).toContain("the session whose pid matches in `maestro status --live`");
    expect(lane).toContain("Codex runs SessionStart on its first turn");
    expect(lane).toContain("without `--target-session`");
    expect(lane).toContain("`claimed by` or `held by` equals the pane's session");
    expect(lane).not.toContain("Reply with the single word");
    expect(lane).not.toContain("ask the started lane");
    expect(lane).toContain(
      "Never send a warm-up prompt just to learn the id.",
    );
    expect(lane).toContain("Never treat the pane id as session identity.");
    // Both stores allocate x-ids from 1, so the room's ids will eventually
    // collide with a repository's; cwd is the only thing that tells them apart
    // and no verb takes a store argument (d717).
    expect(lane).toContain(
      "A lane's cwd alone decides which store every Maestro verb reads",
    );
    expect(lane).toContain("A room-store lane is opened with `--cwd ~/maestro`");
    expect(lane).toContain(
      "compares the contract from `maestro dispatch show <dispatch-id>` against the contract in its prompt",
    );
    expect(commands).toEqual([
      'maestro work add "<title>" --atomic-reason "<why>"',
      "maestro work release <work-id>",
      'maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id>',
      "maestro recipe show slp",
      "maestro dispatch show <dispatch-id>",
      "maestro dispatch list <work-id>",
      "maestro dispatch accept <dispatch-id>",
      "maestro status --live",
      "maestro dispatch confirm <dispatch-id> --session <session-id>",
      "maestro dispatch cancel <dispatch-id> --reason wrong-holder",
      'maestro handback file <dispatch-id> --status DONE --candidate "<commit or digest>" --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"',
      'maestro work note <work-id> "after h<id>: <evidence>"',
      "maestro brief",
      'maestro handback review <handback id> --note "<what you decided>"',
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
      ["<commit or digest>", "candidate-sha"],
      ["<current belief>", "commands parse"],
      ["<what you decided>", "read it; closing the card next"],
    ]);
    const argumentsFor = (command: string): string[] => {
      if (command === "maestro work note") return ["work", "note", "--help"];
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
        const work = idFrom(parsed);
        replacements.set("<work-id>", work);
        expect(
          (await runInstalledCliAt(fixture, fixture.repo, ["work", "start", work], { PATH: path }))
            .exitCode,
        ).toBe(0);
      }
      if (command.startsWith("maestro dispatch open ")) {
        replacements.set("<dispatch-id>", parsed.stdout.match(/^(x\d+)/)?.[1] as string);
      }
      if (command.startsWith("maestro handback file ")) {
        replacements.set("<handback id>", parsed.stdout.match(/^(h\d+)/)?.[1] as string);
      }
      if (command.startsWith("maestro dispatch cancel ")) {
        const openCommand = commands.find((candidate) =>
          candidate.startsWith("maestro dispatch open ")
        ) as string;
        const reopened = await runInstalledCliAt(
          fixture,
          fixture.repo,
          argumentsFor(openCommand),
          { PATH: path },
        );
        expect(reopened.exitCode).toBe(0);
        const dispatch = reopened.stdout.match(/^(x\d+)/)?.[1] as string;
        replacements.set("<dispatch-id>", dispatch);
        expect(
          (
            await runInstalledCliAt(
              fixture,
              fixture.repo,
              ["dispatch", "accept", dispatch],
              { PATH: path },
            )
          ).exitCode,
        ).toBe(0);
        expect(
          (
            await runInstalledCliAt(
              fixture,
              fixture.repo,
              ["dispatch", "confirm", dispatch, "--session", "test-session"],
              { PATH: path },
            )
          ).exitCode,
        ).toBe(0);
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

test("275 [lint] the installed room overlays Herdr agent status on work-scoped lane rows once", async () => {
  // Shell-boundary lint: proves overlay behavior for fixture envelopes, not compatibility with real Herdr output.
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
      `${await realpath(fixture.repo)}: attention REPEATED_FAILURE work ${alpha}`,
    );
    expect(brief.stdout).toContain(
      `${await realpath(secondRepo)}: attention REPEATED_FAILURE work ${beta}`,
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
    expect(brief.stdout).toContain(`skipped: ${deletedPath} (missing)`);
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
      `${await realpath(fixture.repo)}: attention DECISION_STALE decision ${decision}`,
    );
    expect(brief.stdout).not.toContain("ordinary decision progress");
  });
});

test("418 brief reports HUMAN_DECISION_REQUIRED and omits ordinary progress", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary decision progress");
    const work = await addBriefWork(fixture, fixture.repo, path, "owner decision needed");
    const decision = idFrom(
      await runInstalledCliAt(
        fixture,
        fixture.repo,
        ["decision", "draft", "choose owner boundary", "--needs-owner", "--work", work],
        { PATH: path },
      ),
    );

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention HUMAN_DECISION_REQUIRED decision ${decision}`,
    );
    expect(brief.stdout).not.toContain("ordinary decision progress");
  });
});

test("431 brief reports DECISION_REVIEW_DUE and omits ordinary progress", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary decision progress");
    const work = await addBriefWork(fixture, fixture.repo, path, "decision review due");
    const decision = idFrom(
      await runInstalledCliAt(
        fixture,
        fixture.repo,
        [
          "decision",
          "draft",
          "review the accepted boundary",
          "--review-at",
          new Date(Date.now() - 60_000).toISOString(),
          "--work",
          work,
        ],
        { PATH: path },
      ),
    );
    expect(
      (await runInstalledCliAt(
        fixture,
        fixture.repo,
        ["decision", "lock", decision],
        { PATH: path },
      )).exitCode,
    ).toBe(0);

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention DECISION_REVIEW_DUE decision ${decision}`,
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
      `${await realpath(fixture.repo)}: attention STALLED_LEASE work ${work}`,
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
      `${await realpath(fixture.repo)}: attention REPEATED_FAILURE work ${work}`,
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
    const recentWork = await addBriefWork(fixture, fixture.repo, path, "recent lane");
    const recentOpened = await runInstalledCliAt(
      fixture,
      fixture.repo,
      [
        "dispatch",
        "open",
        recentWork,
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
        "w1:pY",
      ],
      { PATH: path },
    );
    expect(recentOpened.exitCode).toBe(0);
    const recent = recentOpened.stdout.match(/^(x\d+)/)?.[1] as string;
    const database = openRepoDatabase(fixture.repo);
    // The default threshold is two hours: 2h30 is stale, 1h30 is not.
    database.query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 150 * 60_000).toISOString(), dispatch);
    database.query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 90 * 60_000).toISOString(), recent);
    database.close();

    const brief = await runInstalledCliAt(
      fixture,
      join(fixture.home, "maestro"),
      ["brief"],
      { MAESTRO_READ_ONLY: "1", PATH: path },
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      `${await realpath(fixture.repo)}: attention DISPATCH_UNRETURNED dispatch ${dispatch}`,
    );
    expect(brief.stdout).not.toContain("ordinary dispatch progress");
    expect(brief.stdout).not.toContain(`DISPATCH_UNRETURNED dispatch ${recent}`);
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
      `${await realpath(fixture.repo)}: attention SCOPE_COLLISION work ${first},${second}`,
    );
    expect(brief.stdout).not.toContain("ordinary collision progress");
  });
});

test("415 brief reports LEAD_COLLISION and omits ordinary in-progress work", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    await addBriefWork(fixture, fixture.repo, path, "ordinary Lead progress");
    const first = await addBriefWork(fixture, fixture.repo, path, "first Lead scope");
    const second = await addBriefWork(fixture, fixture.repo, path, "second Lead scope");
    for (const [work, holder] of [[first, "lead-a"], [second, "lead-b"]] as const) {
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
      `${await realpath(fixture.repo)}: attention LEAD_COLLISION work ${first},${second}`,
    );
    expect(brief.stdout).not.toContain("ordinary Lead progress");
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

test("442 install preserves review-date frontmatter in every method skill", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    for (const name of ["maestro-bundle", "maestro-design", "maestro-work", "maestro-verify"]) {
      const skill = await readFile(
        join(fixture.home, "maestro", "skills", name, "SKILL.md"),
        "utf8",
      );
      const frontmatter = skill.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1] ?? "";
      const source = await readFile(
        join(import.meta.dir, "..", "src", "plugins", "skills", name, "SKILL.md"),
        "utf8",
      );
      const sourceDate = source.match(/^review-date: (\d{4}-\d{2}-\d{2})$/m)?.[1];
      expect(sourceDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(frontmatter).toContain(`review-date: ${sourceDate}`);
    }
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

test("492 install records and backfills one idempotent room-store fact", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const room = join(fixture.home, "maestro");

    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(roomMetaRows(room)).toEqual([{ key: "kind", value: "room" }]);

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(roomMetaRows(room)).toEqual([{ key: "kind", value: "room" }]);

    const database = openRepoDatabase(room);
    database.query("DELETE FROM meta WHERE key = 'kind'").run();
    database.close();
    expect(roomMetaRows(room)).toEqual([]);

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(roomMetaRows(room)).toEqual([{ key: "kind", value: "room" }]);
  });
});

test("493 isRoom rejects repository and unmarked bare stores", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const roomModule = await import("../src/plugins/room.ts") as unknown as {
      isRoom?: (database: Database) => boolean;
    };
    expect(roomModule.isRoom).toBeFunction();

    const repository = openRepoDatabase(fixture.repo);
    expect(roomModule.isRoom?.(repository)).toBe(false);
    repository.close();

    const bareRoom = join(fixture.home, "maestro");
    await mkdir(join(bareRoom, ".maestro"), { recursive: true });
    new Database(join(bareRoom, ".maestro", "maestro.db")).close();
    const bare = openRepoDatabase(bareRoom);
    expect(roomModule.isRoom?.(bare)).toBe(false);
    bare.close();
  });
});

test("494 room mark refuses an ordinary repository command path", async () => {
  await withFixture(async (fixture) => {
    const marked = await runCli(fixture, ["room", "mark"]);

    expect(marked.exitCode).toBe(1);
    expect(JSON.parse(marked.stderr)).toMatchObject({
      error: {
        code: "ROOM_MARK_INTERNAL",
        message: "room mark is reserved for the room-scaffolding code path",
      },
    });
    const database = openRepoDatabase(fixture.repo);
    const count = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM meta WHERE key = 'kind'")
      .get()?.count;
    database.close();
    expect(count).toBe(0);
  });
});

test("495 repository wiring verbs refuse a marked room without changing it", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const files = [
      "AGENTS.md",
      "CLAUDE.md",
      "OWNER.md",
      ".claude/settings.json",
    ];
    const beforeFiles = new Map(
      await Promise.all(
        files.map(async (name) => [name, await readFile(join(room, name), "utf8")] as const),
      ),
    );
    const beforeStore = (await storeSnapshot(room)).map(([name, , content]) => [name, content]);
    const message =
      "~/maestro is the Supervisor room, not a repository; run maestro install from a repository checkout, which maintains the room";

    for (const verb of ["install", "update", "uninstall"]) {
      const refused = await runInstalledCliAt(fixture, room, [verb], { PATH: path });
      expect(refused.exitCode).toBe(1);
      expect(JSON.parse(refused.stderr)).toMatchObject({
        error: { code: "INSTALL_IN_ROOM", message },
      });
      for (const name of files) {
        expect(await readFile(join(room, name), "utf8")).toBe(beforeFiles.get(name)!);
      }
      expect((await storeSnapshot(room)).map(([name, , content]) => [name, content])).toEqual(
        beforeStore,
      );
    }
  });
});

test("496 room guidance names repository-only verbs and the lane return boundary", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const repositoryOnly =
      "Repository-only verbs are `maestro install`, `maestro update`, and `maestro uninstall`; `maestro doctor` wiring checks describe repositories, not this room.";
    for (const name of ["AGENTS.md", "CLAUDE.md"]) {
      expect(await readFile(join(room, name), "utf8")).toContain(repositoryOnly);
    }
    const lane = await readFile(join(room, "lane.md"), "utf8");
    expect(lane).not.toContain("never talks to the Lead through the terminal");
    expect(lane).toContain("`herdr pane send-text`");
    expect(lane).toContain(
      '`herdr agent prompt lead-<repo basename> "[from peer][x<id>] <message>"`',
    );
    expect(lane).toContain("about a stored record (a handback, a note, a draft)");
    expect(lane).toContain("Its returns stay the handback and `--request`.");
    expect(lane).toContain("A lane never messages the Supervisor.");
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
  "236 [lint] hm prints the read-only brief and starts supervisor in an unoccupied pane",
  async () => {
    // Shell-boundary lint: proves the generated function's fake-command path, not real Herdr effects.
    const runHm = async (workspaceExists: boolean) =>
      withFixture(async (fixture) => {
        const { path } = await prepareInstallFixture(fixture);
        await runCli(fixture, ["install"], { PATH: path });
        const fakeBin = join(fixture.root, "fake-bin-brief");
        const herdrLog = join(fixture.root, "herdr-brief.log");
        const maestroLog = join(fixture.root, "maestro-brief.log");
        const workspaceState = join(fixture.root, "herdr-workspace-state");
        await mkdir(fakeBin, { recursive: true });
        if (workspaceExists) await writeFile(workspaceState, "occupied root pane");
        await writeFile(
          join(fakeBin, "herdr"),
          `#!/bin/sh
printf '%s\\n' "$*" >> "$HERDR_LOG"
case "$1 $2" in
  "workspace list")
    if [ -f "$HERDR_WORKSPACE_STATE" ]; then
      printf '%s\\n' '{"id":"test","result":{"type":"workspace_list","workspaces":[{"label":"maestro","workspace_id":"w9"}]}}'
    else
      printf '%s\\n' '{"id":"test","result":{"type":"workspace_list","workspaces":[]}}'
    fi
    ;;
  "workspace create")
    : > "$HERDR_WORKSPACE_STATE"
    printf '%s\\n' '{"id":"test","result":{"workspace":{"label":"maestro","workspace_id":"w9"}}}'
    ;;
  "workspace focus")
    printf '%s\\n' '{"id":"test","result":{"workspace_id":"w9"}}'
    ;;
  "pane list")
    printf '%s\\n' '{"id":"test","result":{"type":"pane_list","panes":[{"pane_id":"w9:p1","workspace_id":"w9"}]}}'
    ;;
  "agent list")
    printf '%s\\n' '{"id":"test","result":{"type":"agent_list","agents":[]}}'
    ;;
  "tab create")
    printf '%s\\n' '{"id":"test","result":{"tab":{"tab_id":"w9:t2"},"root_pane":{"pane_id":"w9:t2:p1","workspace_id":"w9"}}}'
    ;;
  "agent start")
    printf '%s\\n' '{"id":"test","result":{"name":"supervisor"}}'
    ;;
esac
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
              HERDR_WORKSPACE_STATE: workspaceState,
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
        return {
          commands: (await readFile(herdrLog, "utf8")).trim().split("\n"),
          room: join(fixture.home, "maestro"),
        };
      });

    const occupiedWorkspace = await runHm(true);
    expect(occupiedWorkspace.commands).toEqual([
      "workspace list",
      "workspace focus w9",
      "pane list --workspace w9",
      "agent list",
      `tab create --workspace w9 --cwd ${occupiedWorkspace.room} --label supervisor`,
      "agent start supervisor --kind claude --pane w9:t2:p1 -- --autocompact 250000",
    ]);
    expect(occupiedWorkspace.commands).not.toContain(
      "agent start supervisor --kind claude --pane w9:p1 -- --autocompact 250000",
    );

    const freshWorkspace = await runHm(false);
    expect(freshWorkspace.commands).toEqual([
      "workspace list",
      `workspace create --cwd ${freshWorkspace.room} --label maestro --focus`,
      "workspace list",
      "pane list --workspace w9",
      "agent list",
      "agent start supervisor --kind claude --pane w9:p1 -- --autocompact 250000",
    ]);
  },
);

test("412 first install seeds OWNER.md with the interview questions and the room asks them before the brief", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const owner = await readFile(join(room, "OWNER.md"), "utf8");
    const questions = owner.split("\n").filter((line) => /^  - .*\?$/.test(line));

    expect(questions.length).toBeGreaterThanOrEqual(6);
    expect(owner).toContain("never overwritten");
    for (const name of ["AGENTS.md", "CLAUDE.md"]) {
      const text = await readFile(join(room, name), "utf8");
      expect(text).toContain("interview the owner");
      expect(text.indexOf("interview the owner")).toBeLessThan(text.indexOf("maestro brief"));
    }
  });
});

test("529 [lint] relayed intent reports on close even without a room decision id (d22)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lead = await readFile(join(fixture.home, "maestro", "lead.md"), "utf8");
    const slp = await readFile(
      join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
      "utf8",
    );
    const step6 = lead.split("\n6. ")[1]?.split("\n7. ")[0] ?? "";
    expect(step6).not.toBe("");

    // Keying the report on a room decision id missed room card w4 to
    // lead-dotfiles: that relay carried intent and named no decision, so the
    // trigger never fired and the closure went unreported.
    expect(step6).toContain("[from supervisor][intent]");
    expect(step6).toContain("w<room-id>");
    expect(step6).toContain("d<room-id>");
    expect(slp).toContain("w<room-id>");

    // The same Lead then hunted for the room with ListAgents and dispatch
    // list. The room agent name is fixed, so searching is always wrong.
    expect(step6).toContain("herdr agent prompt supervisor");
    expect(step6).toMatch(/only channel|reached only by/);
    expect(step6).toContain("herdr agent list");
    expect(step6).toContain("maestro dispatch list");
  });
});

function flat(text: string): string {
  return text.replace(/\s+/g, " ");
}

test("533 [lint] the slp recipe carries the d28 team model", async () => {
  const raw = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const slp = flat(raw);

  // A team is a workspace, and the two coordinates are read from different
  // places: role from the name prefix, team from the workspace id. cwd decides
  // which store a verb reads (d717) and never which team a pane belongs to.
  expect(slp).toContain("A team is one Herdr workspace");
  expect(slp).toContain("role from the name prefix, team from the workspace id, never from cwd");

  // The three team-scoped names and what each one may not do.
  expect(slp).toContain("supervisor-<team>");
  expect(slp).toContain("advisor-<team>");
  expect(slp).toContain("observer-<team>");
  expect(slp).toContain("consult-<repo basename>");
  expect(slp).toContain("exactly one record holder");

  // The observer speaks, it never acts: no assignment change, no freeze, no
  // write verb, no store write, and only inside its own workspace.
  expect(slp).toContain(
    "[from observer][suspected] <pane> <quoted evidence> <why>",
  );
  expect(slp).toContain("its own workspace only");
  expect(slp).toContain(
    "never changes an assignment, never freezes, never runs a write verb, never writes the store",
  );
  expect(slp).toContain("once per issue and again only on new evidence");

  // Triggers are countable so a drift call is checkable rather than a matter
  // of taste.
  const triggers = raw.match(/Triggers are countable[\s\S]*?\n\n/)?.[0] ?? "";
  expect(triggers).not.toBe("");
  for (const trigger of [
    "the same failure a third time",
    "contradicting",
    "a role answering a question type it does not own",
    "silent past its stop condition",
    "self-doubt phrases repeated in one turn",
  ]) {
    expect(flat(triggers), trigger).toContain(trigger);
  }

  // d719: the report target is a parameter the opener names, not a fixed name.
  expect(slp).toContain("[from lead][done w<id> re <room record>]");
  expect(slp).toContain(
    "the record holder named in the prompt that opened it",
  );
});

test("534 [lint] installed lead guidance opens a team workspace before any pane (d29)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lead = flat(await readFile(join(fixture.home, "maestro", "lead.md"), "utf8"));

    // The room's own workspace stays clean: every pane the room opens goes to
    // the team's workspace, which the room creates first.
    expect(lead).toContain(
      "`herdr workspace create --cwd <team cwd> --label team-<name> --no-focus`",
    );
    expect(lead).toContain("never in the room's own workspace");

    // One team cwd is one workspace: read the list and reuse before creating.
    expect(lead).toContain("`herdr workspace list`");
    expect(lead).toContain("reuse");
    expect(lead).toContain("never opens a duplicate");

    // d719: the relay prompt carries the report target, so the Lead never
    // searches for it.
    expect(lead).toContain("report to <record holder>");
  });
});

test("535 [lint] the room's transcript denial stays while observer-<team> reads its own workspace", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const identity = flat(await readFile(join(room, "IDENTITY.md"), "utf8"));
    const agents = flat(await readFile(join(room, "AGENTS.md"), "utf8"));

    // The room still reads stores, not panes. The d28 grant belongs to a
    // different role in a different workspace and does not widen this binding.
    expect(identity).toContain("Raw transcript access: denied");
    expect(identity).toContain("observer-<team>");
    expect(identity).toContain("does not widen this binding");

    // d29: the room supervisor opens no agent in the room's own workspace.
    expect(agents).toContain("opens no agent in this workspace");
    expect(agents).toContain("the owner may open their own");
  });
});

test("536 [lint] the room scaffolds the observer watcher and its start template (d33)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const watcher = await readFile(join(room, "observer-watch.sh"), "utf8");
    const observer = flat(await readFile(join(room, "observer.md"), "utf8"));
    const lead = flat(await readFile(join(room, "lead.md"), "utf8"));

    // The watcher is a sensor: it wakes the model, it never judges.
    expect(watcher).toContain("herdr agent wait");
    expect(watcher).toContain("--source recent-unwrapped");
    expect(watcher).toContain("[watch] $name");
    expect(watcher).toContain("LEASE_HELD");
    expect(watcher).toContain("not my role");
    expect(flat(watcher)).toContain("Sensor, not judgment");

    // It belongs to the observer's pane, not to maestro: this is what keeps
    // the deleted supervisor daemon deleted.
    expect(flat(watcher)).toContain("No maestro verb starts it");
    expect(watcher).toContain("HERDR_WORKSPACE_ID");

    // Counting is what makes a trigger countable rather than a matter of taste.
    expect(watcher).toContain("doubt >= 2");
    expect(watcher).toContain("seen[$0] == 3");

    // The start template tells the room how to open the observer, and the
    // observer what to do when the watcher wakes it.
    expect(observer).toContain("herdr agent start observer-<team>");
    expect(observer).toContain("observer-watch.sh observer-<team>");
    expect(observer).toContain("[from observer][suspected]");
    expect(observer).toContain("once per issue");
    expect(observer).toContain("ledger");
    expect(observer).toContain("never writes the store");

    // A bare maestro command creates the store where none exists and refreshes
    // a session row, so every store read the observer makes carries the prefix
    // or the template contradicts itself.
    const reads = [...observer.matchAll(/(\S*\s?)maestro (status|work show)/g)];
    expect(reads.length).toBeGreaterThan(0);
    for (const read of reads) expect(read[1]).toContain("MAESTRO_READ_ONLY=1");

    // The executable bit matters: the observer runs this, it does not read it.
    const mode = (await stat(join(room, "observer-watch.sh"))).mode & 0o777;
    expect(mode & 0o100).toBe(0o100);

    // Opening the observer is part of opening a team.
    expect(lead).toContain("observer.md");
  });
});

test("537 [lint] a misrouted [from lead] prompt is bounced, not processed (d35)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const identity = flat(await readFile(join(room, "IDENTITY.md"), "utf8"));
    const lead = flat(await readFile(join(room, "lead.md"), "utf8"));
    const slp = flat(
      await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8"),
    );

    const bounce = "not my supervisor: send to supervisor-<team>";
    for (const [name, text] of [
      ["IDENTITY.md", identity],
      ["lead.md", lead],
      ["slp.md", slp],
    ] as const) {
      expect(text, name).toContain(bounce);
      // Fail closed: the bounce replaces verification and the record, it does
      // not precede them.
      expect(text, name).toContain("neither verified nor recorded");
      // Ownership is a workspace fact, never a cwd one.
      expect(text, name).toContain("workspace_id");
    }

    // The rule is symmetric: a team supervisor bounces a foreign Lead too.
    expect(slp).toContain("a Lead outside its workspace");
  });
});

test("538 [lint] a team reaches the room only through supervisor-<team> (d36)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const identity = flat(await readFile(join(room, "IDENTITY.md"), "utf8"));
    const slp = flat(
      await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8"),
    );

    const channel =
      "herdr agent prompt supervisor \"[from supervisor-<team>][report|ask|done w<room-id>] ...\"";
    expect(slp).toContain(channel);
    expect(identity).toContain(channel);

    // One channel up, and only one: everyone else in the team is silent
    // towards the room.
    for (const text of [slp, identity]) {
      expect(text).toContain("the only prompt crossing a workspace boundary upward");
      expect(text).toContain(
        "Leads, advisors, observers and peers never prompt the room",
      );
      expect(text).toContain("only through its `supervisor-<team>`");
    }

    // The d35 exception is the same one: a Lead the room opened and still owns.
    expect(slp).toContain("the room itself opened and still owns");
  });
});

test("539 [lint] the room holds owner authority in full behind the external-effect gate (d37)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const identity = flat(await readFile(join(room, "IDENTITY.md"), "utf8"));
    const agents = flat(await readFile(join(room, "AGENTS.md"), "utf8"));
    const claude = await readFile(join(room, "CLAUDE.md"), "utf8");
    const slp = flat(
      await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8"),
    );

    // Full equivalence: the three binding lines that used to read "none" now
    // carry the owner's own authority.
    expect(identity).toContain("Write authority: the owner's, in full");
    expect(identity).toContain("Acceptance authority: the owner's, at the owner boundary");
    expect(identity).toContain("Recovery or replacement lease: standing, in any team");
    for (const effect of ["push", "tag", "release", "publish", "deploy", "`maestro update`"]) {
      expect(identity).toContain(effect);
    }
    expect(identity).not.toContain("Write authority: none");

    // Authority without the gate is the failure this replaces: an external
    // effect still needs a locked decision, a named candidate, and a record.
    for (const text of [identity, slp]) {
      expect(text).toContain("never straight from a Lead's prompt");
    }
    expect(identity).toContain("names the exact candidate and the verified evidence");

    // Intervention reaches any team, but a code correction still goes through
    // that team's Lead: the room does not become a second Lead.
    expect(slp).toContain("It may intervene in any team to stop or correct an error");
    expect(slp).toContain("redirect or replace a `supervisor-<team>` or a Lead");
    for (const text of [identity, slp]) {
      expect(text).toContain("unless the room explicitly takes a lane over");
    }

    // The Human section binds the room too, so a rule written for one seat is
    // not read as exempting the other.
    expect(slp).toContain("Every authority in this section is also the Supervisor's");

    // AGENTS.md and CLAUDE.md are the same generated text and both say it.
    expect(agents).toContain("It holds the owner's authority in full");
    expect(claude).toBe(await readFile(join(room, "AGENTS.md"), "utf8"));
  });
});

test("553 [lint] the room renders the project view and hands it to a new Lead (w552/d42)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const lead = flat(await readFile(join(fixture.home, "maestro", "lead.md"), "utf8"));

    // The view is rendered before it is handed, so the Lead reads it fresh.
    expect(lead).toContain("`maestro lesson render`");
    expect(lead).toContain("~/maestro/PROJECT/<repo basename>.md");
    expect(lead).toContain("before your first card");
  });
});
