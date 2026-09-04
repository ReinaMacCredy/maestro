import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { chmod, mkdir, readFile, readdir, realpath, rm, stat, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { formatSkillSync, materializeSkills, skillNames } from "../src/plugins/skills.ts";
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

test("267 reinstall preserves OWNER.md and the Hub SLP pack while refreshing managed room files", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");
    const generatedNames = [
      "IDENTITY.md",
      "AGENTS.md",
      "CLAUDE.md",
      "shellrc",
      "WORKFLOW.md",
      ".claude/settings.json",
    ];
    const firstInstall = new Map(
      await Promise.all(
        generatedNames.map(async (name) => [name, await readFile(join(room, name), "utf8")] as const),
      ),
    );
    const ownerEdit = "# OWNER\n\nOwner-authored content survives installs.\n";
    const packEdit = `${await readFile(join(room, "SLP.md"), "utf8")}\nOwner pack edit.\n`;
    await writeFile(join(room, "OWNER.md"), ownerEdit);
    await writeFile(join(room, "SLP.md"), packEdit);
    for (const retired of ["lane.md", "lead.md", "observer.md", "supervisor.md", "observer-watch.sh"]) {
      await writeFile(join(room, retired), `stale ${retired}\n`);
    }
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
    expect(await readFile(join(room, "SLP.md"), "utf8")).toBe(packEdit);
    for (const name of generatedNames) {
      expect(await readFile(join(room, name), "utf8")).toBe(firstInstall.get(name) ?? "");
      expect(await readFile(join(room, name), "utf8")).not.toContain("stale ");
    }
    for (const retired of ["lane.md", "lead.md", "observer.md", "supervisor.md", "observer-watch.sh"]) {
      expect(existsSync(join(room, retired))).toBe(false);
    }
  });
});

test("268 first install seeds a neutral OWNER.md", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const owner = await readFile(join(fixture.home, "maestro", "OWNER.md"), "utf8");

    expect(owner).toContain("machine constraints");
    expect(owner).toContain("communication preferences");
    expect(owner).toContain("maestro decide");
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
    for (const name of ["maestro-bundle", "maestro-design", "maestro-improve", "maestro-work", "maestro-verify"]) {
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

test("577 materializeSkills links every skill for Codex, relinks a dangling link, keeps an unmanaged dir", async () => {
  await withFixture(async (fixture) => {
    const codexSkills = join(fixture.home, ".codex", "skills");
    await mkdir(join(codexSkills, "maestro-work"), { recursive: true });
    await writeFile(join(codexSkills, "maestro-work", "SKILL.md"), "---\nname: maestro-work\n---\nowner's own\n");
    await symlink(join(fixture.home, ".maestro", "skills", "maestro-design"), join(codexSkills, "maestro-design"));

    const sync = await materializeSkills(fixture.home, "abc1234");

    const others = skillNames.filter((name) => name !== "maestro-work");
    expect(sync.linked).toEqual([...skillNames]);
    expect(sync.linkedCodex).toEqual(others);
    expect(sync.linkSkippedCodex).toEqual(["maestro-work"]);
    for (const name of others) {
      expect(await realpath(join(codexSkills, name))).toBe(
        await realpath(join(fixture.home, "maestro", "skills", name)),
      );
    }
    expect((await stat(join(codexSkills, "maestro-work"))).isDirectory()).toBe(true);
    expect(await readFile(join(codexSkills, "maestro-work", "SKILL.md"), "utf8")).toContain("owner's own");
    expect(formatSkillSync(sync)).toContain(`skills linked for Codex: ${others.join(", ")}`);
    expect(formatSkillSync(sync)).toContain("skill link skipped: maestro-work (unmanaged Codex skill");
  });
});

test("442 install preserves review-date frontmatter in every method skill", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    for (const name of ["maestro-bundle", "maestro-design", "maestro-improve", "maestro-work", "maestro-verify"]) {
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

test("242 owner preferences are immutable Hub decisions whose replacements preserve history", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const room = join(fixture.home, "maestro");
    const owner = await readFile(join(room, "OWNER.md"), "utf8");
    expect(owner).toContain("maestro decide");
    expect(owner).toContain("--replaces");

    const first = await runInstalledCliAt(
      fixture,
      room,
      [
        "decide",
        "Prefer terse project briefs",
        "--why",
        "the owner asked for less noise",
      ],
      { PATH: path },
    );
    const firstId = idFrom(first);
    const reversal = await runInstalledCliAt(
      fixture,
      room,
      [
        "decide",
        "Prefer detailed project briefs",
        "--why",
        "the owner reversed the earlier preference",
        "--replaces",
        firstId,
      ],
      { PATH: path },
    );
    const reversalId = idFrom(reversal);
    const database = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      database
        .query<{ id: string; replaces_id: string | null }, []>(
          "SELECT id, replaces_id FROM slp_decisions ORDER BY id",
        )
        .all(),
    ).toEqual([
      { id: firstId, replaces_id: null },
      { id: reversalId, replaces_id: firstId },
    ]);
    database.close();
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
      "~/maestro is the Hub, not a repository; run maestro install from a repository checkout, which maintains the Hub";

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
