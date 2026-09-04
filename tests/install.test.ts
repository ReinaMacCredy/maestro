import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { chmod, cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { resolveHomeDirectory } from "../src/plugins/home.ts";
import { idFrom, prepareInstallFixture, runCli, runInstalledCliAt, withFixture } from "./helpers.ts";

const roomTrustPrefix = "room Codex setup:";
const shellSourceLine =
  '[[ -f "$HOME/maestro/shellrc" ]] && source "$HOME/maestro/shellrc" # maestro';

test("282 first install needs no rollback binary and targets the detected shell", async () => {
  const cleanPath = [dirname(process.execPath), "/usr/bin", "/bin"].join(":");

  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"], {
      PATH: cleanPath,
      SHELL: "/bin/zsh",
    });

    expect(installed.exitCode).toBe(0);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro")).exists()).toBe(true);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-slp-watch")).exists()).toBe(true);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-team-sensor")).exists()).toBe(false);
  });

  await withFixture(async (fixture) => {
    const environment = { PATH: cleanPath, SHELL: "/bin/bash" };
    expect((await runCli(fixture, ["install"], environment)).exitCode).toBe(0);
    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], environment)).exitCode,
    ).toBe(0);

    const bashrc = await readFile(join(fixture.home, ".bashrc"), "utf8");
    expect(bashrc.split("\n").filter((line) => line === shellSourceLine)).toHaveLength(1);
    expect(await Bun.file(join(fixture.home, ".zshrc")).exists()).toBe(false);
  });

  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"], {
      PATH: cleanPath,
      SHELL: "/usr/bin/fish",
    });

    expect(installed.exitCode).toBe(0);
    expect(await Bun.file(join(fixture.home, ".zshrc")).exists()).toBe(false);
    expect(await Bun.file(join(fixture.home, ".bashrc")).exists()).toBe(false);
    expect(installed.stdout).toContain(shellSourceLine);
  });
}, 20_000);

test("439 install warns about a live dispatch holder and still completes", async () => {
  await withFixture(async (fixture) => {
    const peer = {
      MAESTRO_SESSION_ID: "activation-install-peer",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], peer)).exitCode,
    ).toBe(0);
    const added = await runCli(
      fixture,
      [
        "work",
        "add",
        "live install dispatch",
        "--atomic-reason",
        "fixture",
      ],
      peer,
    );
    expect(added.exitCode).toBe(0);
    const work = idFrom(added);
    const opened = await runCli(
      fixture,
      [
        "dispatch",
        "open",
        work,
        "--objective",
        "hold a live dispatch during install",
        "--owned-scope",
        "fixture",
        "--excluded-scope",
        "product source",
        "--mutation",
        "no-write",
        "--stop-condition",
        "install completes",
        "--lane",
        "scout",
        "--evidence-required",
        "source: fixture",
        "--pane",
        "fixture:p1",
        "--target-session",
        peer.MAESTRO_SESSION_ID,
      ],
      peer,
    );
    expect(opened).toEqual(expect.objectContaining({ exitCode: 0 }));
    const dispatch = opened.stdout.match(/^x\d+/)?.[0];
    if (!dispatch) throw new Error(`missing dispatch id in stdout: ${opened.stdout}`);
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], peer)).exitCode).toBe(0);
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], {
      MAESTRO_SESSION_ID: "activation-installer",
      PATH: path,
    });

    expect(installed.exitCode).toBe(0);
    expect(installed.stderr).toContain(
      `[install] 1 live session holds work or an open dispatch (repos: ${fixture.repo}); they load the new runtime on their next maestro call`,
    );
  });
});

test("SLP v2 install refuses runtime replacement while a v1 team is live", async () => {
  await withFixture(async (fixture) => {
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.exec(`
      CREATE TABLE team_lifecycle (
        team_id TEXT PRIMARY KEY,
        generation INTEGER NOT NULL,
        stage TEXT NOT NULL
      );
      INSERT INTO team_lifecycle (team_id, generation, stage)
      VALUES ('legacy-live', 7, 'ACTIVE');
    `);
    database.close();
    const { path } = await prepareInstallFixture(fixture);
    const runtimeCli = join(fixture.home, ".maestro", "runtime", "bin", "maestro.ts");
    await mkdir(dirname(runtimeCli), { recursive: true });
    await writeFile(runtimeCli, "old runtime bytes\n");

    const blocked = await runCli(fixture, ["install"], { PATH: path });

    expect(blocked.exitCode).toBe(1);
    expect(blocked.stderr).toContain('"code":"SLP_V1_TEAM_RUNNING"');
    expect(blocked.stderr).toContain("legacy-live:g7");
    expect(await readFile(runtimeCli, "utf8")).toBe("old runtime bytes\n");

    const stopped = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    stopped.query("UPDATE team_lifecycle SET stage = 'STOPPED'").run();
    stopped.close();
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(await readFile(runtimeCli, "utf8")).not.toBe("old runtime bytes\n");
  });
}, 30_000);

test("A5 / B3.9 install preserves rollback and writes harness-specific adapters", async () => {
  await withFixture(async (fixture) => {
    const legacySource = "#!/bin/sh\necho legacy-maestro\n";
    const { localBin, path, shim } = await prepareInstallFixture(fixture, legacySource);
    const legacy = join(localBin, "maestro-legacy");

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).toContain("review Codex hook trust with /hooks");
    expect(await readFile(legacy, "utf8")).toBe(legacySource);
    expect(await readFile(shim, "utf8")).toContain(".maestro/runtime/bin/maestro.ts");
    expect(await Bun.file(join(fixture.home, ".maestro", "runtime", "bin", "maestro.ts")).exists()).toBe(true);

    const config = JSON.parse(
      await readFile(join(fixture.repo, ".maestro", "config"), "utf8"),
    ) as { plugins: Array<{ disabled: boolean; name: string }> };
    expect(config.plugins).toContainEqual({ name: "policy-proof", disabled: false });
    expect(config.plugins).toContainEqual({ name: "policy-breakdown", disabled: false });
    expect(config.plugins).toContainEqual({ name: "policy-tdd", disabled: true });
    expect(config.plugins).toContainEqual({ name: "policy-qa", disabled: true });
    expect(config.plugins).toContainEqual({ name: "policy-research", disabled: true });

    const codexHooks = JSON.parse(
      await readFile(join(fixture.repo, ".codex", "hooks.json"), "utf8"),
    ) as { hooks: Record<string, unknown> };
    const claudeHooks = JSON.parse(
      await readFile(join(fixture.repo, ".claude", "settings.json"), "utf8"),
    ) as { hooks: Record<string, unknown> };
    expect(codexHooks.hooks.SessionStart).toBeArray();
    expect(codexHooks.hooks.UserPromptSubmit).toBeArray();
    expect(claudeHooks.hooks.SessionStart).toBeArray();
    expect(claudeHooks.hooks.UserPromptSubmit).toBeArray();
    expect(existsSync(join(fixture.repo, "AGENTS.md"))).toBe(false);
    expect(existsSync(join(fixture.repo, "CLAUDE.md"))).toBe(false);
    expect(await Bun.file(join(localBin, "maestro-slp-watch")).exists()).toBe(true);

    const codexHooksBefore = await readFile(join(fixture.repo, ".codex", "hooks.json"), "utf8");
    const repeated = await runCli(fixture, ["install"], { PATH: path });
    expect(repeated.exitCode).toBe(0);
    expect(repeated.stdout).not.toContain("review Codex hook trust with /hooks");
    expect(await readFile(join(fixture.repo, ".codex", "hooks.json"), "utf8")).toBe(
      codexHooksBefore,
    );

    for (const [adapter, harness] of [
      [join(fixture.repo, ".claude", "hooks", "maestro-record.ts"), "claude"],
      [join(fixture.repo, ".codex", "hooks", "maestro-record.ts"), "codex"],
    ] as const) {
      const sessionId = `install-${harness}-session`;
      const hook = Bun.spawn([process.execPath, adapter], {
        cwd: fixture.repo,
        env: { ...process.env, HOME: fixture.home, PATH: path },
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
      });
      hook.stdin.write(
        JSON.stringify({
          cwd: fixture.repo,
          hook_event_name: "SessionStart",
          session_id: sessionId,
        }),
      );
      hook.stdin.end();
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(hook.stdout).text(),
        new Response(hook.stderr).text(),
        hook.exited,
      ]);
      expect(stderr).toBe("");
      expect(exitCode).toBe(0);
      expect(stdout).toContain("enabled policies");
      const status = await runCli(fixture, ["status", "--json"]);
      const envelope = JSON.parse(status.stdout) as {
        data: { sessions: Array<{ harness: string | null; id: string }> };
      };
      expect(envelope.data.sessions.find((session) => session.id === sessionId)?.harness).toBe(
        harness,
      );
    }
  });
});

test("270 unverified Codex hook hashes never suppress room trust guidance", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const room = join(fixture.home, "maestro");
    const first = await runCli(fixture, ["install"], { PATH: path });

    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain(
      `${roomTrustPrefix} trust ${room} when Codex asks, then open /hooks and trust both room-local Maestro hooks; start a new Codex session afterward`,
    );
    for (const name of ["AGENTS.md", "CLAUDE.md", "IDENTITY.md", "SLP.md"]) {
      expect(await readFile(join(room, name), "utf8")).not.toContain(roomTrustPrefix);
    }

    const hooksPath = join(room, ".codex", "hooks.json");
    const config =
      `[hooks.state."${hooksPath}:session_start:0:0"]\ntrusted_hash = "sha256:deadbeef"\n` +
      `[hooks.state."${hooksPath}:user_prompt_submit:0:0"]\ntrusted_hash = "sha256:deadbeef"\n`;
    const configPath = join(fixture.home, ".codex", "config.toml");
    await mkdir(join(configPath, ".."), { recursive: true });
    await writeFile(configPath, config);

    const repeated = await runCli(fixture, ["install"], { PATH: path });

    expect(repeated.exitCode).toBe(0);
    expect(repeated.stdout).not.toContain(roomTrustPrefix);
    expect(repeated.stdout).toContain("/hooks");
    expect(repeated.stdout).toContain("Codex has recorded trust for both hooks");
    expect(await readFile(configPath, "utf8")).toBe(config);
  });
});

test("613 machine-scoped paths use an absolute home or fail before writing", async () => {
  expect(
    resolveHomeDirectory({ environmentHome: undefined, fallbackHome: "/os/home" }),
  ).toBe("/os/home");
  for (const invalid of ["", "   ", "relative/home"]) {
    try {
      resolveHomeDirectory({ environmentHome: invalid, fallbackHome: "/os/home" });
      throw new Error("invalid home was accepted");
    } catch (error) {
      expect(error).toEqual(expect.objectContaining({ code: "HOME_REQUIRED" }));
    }
  }

  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    for (const invalid of ["", "relative/home"]) {
      const installed = await runCli(fixture, ["install"], { HOME: invalid, PATH: path });
      expect(installed.exitCode).not.toBe(0);
      expect(installed.stderr).toContain('"code":"HOME_REQUIRED"');
    }
    for (const path of [".local", "maestro", ".zshrc", ".bashrc"]) {
      expect(existsSync(join(fixture.repo, path))).toBe(false);
    }
  });
});

test("29 [lint] install writes portable hook files without machine-absolute paths", async () => {
  await withFixture(async (fixture) => {
    // Proves generated-file portability lint, not execution after relocating the installed fixture.
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], {
      PATH: path,
    });
    const codexHookSource = await readFile(
      join(fixture.repo, ".codex", "hooks", "maestro-record.ts"),
      "utf8",
    );
    const claudeHookSource = await readFile(
      join(fixture.repo, ".claude", "hooks", "maestro-record.ts"),
      "utf8",
    );
    const codexHooks = await readFile(join(fixture.repo, ".codex", "hooks.json"), "utf8");
    const claudeHooks = await readFile(
      join(fixture.repo, ".claude", "settings.json"),
      "utf8",
    );
    const hookFiles = `${codexHookSource}\n${claudeHookSource}\n${codexHooks}\n${claudeHooks}`;

    expect(installed.exitCode).toBe(0);
    expect(hookFiles).not.toContain(fixture.root);
    expect(hookFiles).not.toContain(process.execPath);
    expect(codexHooks).toContain("bun .codex/hooks/maestro-record.ts");
    expect(claudeHooks).toContain("bun .claude/hooks/maestro-record.ts");
    expect(codexHookSource).toContain('"--harness", "codex"');
    expect(claudeHookSource).toContain('"--harness", "claude"');
  });
});

test("45 install relies on harness hooks without creating repository instruction mirrors", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    expect(existsSync(join(fixture.repo, "AGENTS.md"))).toBe(false);
    expect(existsSync(join(fixture.repo, "CLAUDE.md"))).toBe(false);
    expect(existsSync(join(fixture.repo, ".cursor"))).toBe(false);
  });
});

test("436 install preserves the repository Workspace Protocol byte for byte", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const protocols = new Map([
      ["AGENTS.md", "# Agent protocol\n\nuser-owned\n"],
      ["CLAUDE.md", "# Claude protocol\r\nuser-owned\r\n"],
    ]);
    for (const [name, content] of protocols) await writeFile(join(fixture.repo, name), content);
    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
    for (const [name, content] of protocols) {
      expect(await readFile(join(fixture.repo, name), "utf8")).toBe(content);
    }
  });
});

test("614 scripts/install.sh clones the source checkout, installs from it, and fast-forwards on rerun", async () => {
  await withFixture(async (fixture) => {
    const projectRoot = join(import.meta.dir, "..");
    const upstream = join(fixture.root, "upstream");
    await mkdir(upstream, { recursive: true });
    for (const entry of ["package.json", "tsconfig.json", "bin", "src", "scripts", ".gitignore"]) {
      await cp(join(projectRoot, entry), join(upstream, entry), { recursive: true });
    }
    // commit spawns a detached auto-maintenance that repacks loose objects
    // while install.sh is still cloning them; keep the fixture's objects still.
    const git = (args: string[]) =>
      Bun.spawn(["git", "-c", "user.name=Maestro Tests", "-c", "user.email=maestro-tests@example.invalid", "-c", "gc.auto=0", "-c", "maintenance.auto=false", ...args], {
        cwd: upstream,
        stdout: "pipe",
        stderr: "pipe",
      }).exited;
    expect(await git(["init", "-q", "-b", "main"])).toBe(0);
    expect(await git(["add", "."])).toBe(0);
    expect(await git(["commit", "-q", "-m", "upstream"])).toBe(0);

    const { path } = await prepareInstallFixture(fixture);
    const source = join(fixture.home, ".maestro", "source");
    const run = async () => {
      const child = Bun.spawn(["sh", join(projectRoot, "scripts", "install.sh")], {
        cwd: fixture.repo,
        env: {
          ...process.env,
          HOME: fixture.home,
          PATH: path,
          SHELL: "/bin/zsh",
          MAESTRO_REPO: upstream,
          MAESTRO_SESSION_ID: "test-session",
          MAESTRO_SESSION_PID: String(process.pid),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ]);
      return { exitCode: await child.exited, stdout, stderr };
    };

    const first = await run();
    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain(`cloning ${upstream} (main) into ${source}`);
    expect(first.stdout).toContain("herdr.dev/install.sh");
    expect(existsSync(join(source, "bin", "maestro.ts"))).toBe(true);
    const version = await runInstalledCliAt(fixture, fixture.repo, ["version"], { PATH: path });
    expect(version.exitCode).toBe(0);
    expect(version.stdout).toContain(`maestro ${JSON.parse(await readFile(join(projectRoot, "package.json"), "utf8")).version}`);

    const second = await run();
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain(`fast-forwarding the source checkout at ${source}`);
  });
});

test("517 scripts/install.sh pins the newest release tag by version, not main's tip", async () => {
  await withFixture(async (fixture) => {
    const projectRoot = join(import.meta.dir, "..");
    const upstream = join(fixture.root, "upstream");
    await mkdir(upstream, { recursive: true });
    for (const entry of ["package.json", "tsconfig.json", "bin", "src", "scripts", ".gitignore"]) {
      await cp(join(projectRoot, entry), join(upstream, entry), { recursive: true });
    }
    const git = (args: string[]) =>
      Bun.spawn(["git", "-c", "user.name=Maestro Tests", "-c", "user.email=maestro-tests@example.invalid", "-c", "gc.auto=0", "-c", "maintenance.auto=false", ...args], {
        cwd: upstream,
        stdout: "pipe",
        stderr: "pipe",
      });
    const gitOut = async (args: string[]) => (await new Response(git(args).stdout).text()).trim();
    expect(await git(["init", "-q", "-b", "main"]).exited).toBe(0);
    expect(await git(["add", "."]).exited).toBe(0);
    expect(await git(["commit", "-q", "-m", "upstream"]).exited).toBe(0);
    // v0.9.0 sorts ABOVE v0.10.0 lexicographically, so a plain sort picks the
    // older release; only a version-aware comparison gets this right.
    expect(await git(["tag", "v0.9.0"]).exited).toBe(0);
    expect(await git(["commit", "-q", "--allow-empty", "-m", "release"]).exited).toBe(0);
    expect(await git(["tag", "v0.10.0"]).exited).toBe(0);
    const release = await gitOut(["rev-parse", "v0.10.0^{}"]);
    // Mid-flight work lands after the release; an adopter must not get this.
    expect(await git(["commit", "-q", "--allow-empty", "-m", "unreleased work"]).exited).toBe(0);
    const tip = await gitOut(["rev-parse", "HEAD"]);
    expect(release).not.toBe(tip);

    const { path } = await prepareInstallFixture(fixture);
    const source = join(fixture.home, ".maestro", "source");
    const run = async (extra: Record<string, string> = {}) => {
      const child = Bun.spawn(["sh", join(projectRoot, "scripts", "install.sh")], {
        cwd: fixture.repo,
        env: {
          ...process.env,
          HOME: fixture.home,
          PATH: path,
          SHELL: "/bin/zsh",
          MAESTRO_REPO: upstream,
          MAESTRO_SESSION_ID: "test-session",
          MAESTRO_SESSION_PID: String(process.pid),
          ...extra,
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ]);
      return { exitCode: await child.exited, stdout, stderr };
    };
    const sourceGit = async (args: string[]) =>
      (await new Response(Bun.spawn(["git", "-C", source, ...args], { stdout: "pipe", stderr: "pipe" }).stdout).text()).trim();

    const pinned = await run();
    expect(pinned.exitCode).toBe(0);
    expect(await sourceGit(["rev-parse", "HEAD"])).toBe(release);
    // A branch, not a detached HEAD: lifecycle.ts refuses to update a detached
    // checkout, so pinning must not cost the fast-forward contract.
    expect(await sourceGit(["symbolic-ref", "--quiet", "--short", "HEAD"])).toBe("maestro-release");

    const explicit = await run({ MAESTRO_REF: "main", MAESTRO_SOURCE_DIR: join(fixture.home, "dev-source") });
    expect(explicit.exitCode).toBe(0);
    const devHead = (await new Response(
      Bun.spawn(["git", "-C", join(fixture.home, "dev-source"), "rev-parse", "HEAD"], { stdout: "pipe", stderr: "pipe" }).stdout,
    ).text()).trim();
    expect(devHead).toBe(tip);
  });
}, 120_000);

test("312 scripts/install.sh refuses a bun older than the lockfile's bun floor and names it", async () => {
  await withFixture(async (fixture) => {
    const projectRoot = join(import.meta.dir, "..");
    const shims = join(fixture.root, "old-bun");
    await mkdir(shims, { recursive: true });
    await writeFile(join(shims, "bun"), "#!/bin/sh\necho 1.3.14\n");
    await chmod(join(shims, "bun"), 0o755);
    const child = Bun.spawn(["sh", join(projectRoot, "scripts", "install.sh")], {
      cwd: fixture.repo,
      env: { ...process.env, HOME: fixture.home, PATH: `${shims}:${process.env.PATH ?? ""}` },
      stdout: "pipe",
      stderr: "pipe",
    });
    const stderr = await new Response(child.stderr).text();
    expect(await child.exited).toBe(1);
    expect(stderr).toContain("bun 1.3.14 is too old");
    expect(stderr).toContain("bun >= 1.4.0");
  });
});

test("313 scripts/install.sh --help prints usage and exits before touching the machine", async () => {
  await withFixture(async (fixture) => {
    const projectRoot = join(import.meta.dir, "..");
    const run = async (arg: string) => {
      const child = Bun.spawn(["sh", join(projectRoot, "scripts", "install.sh"), arg], {
        cwd: fixture.repo,
        env: { ...process.env, HOME: fixture.home },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ]);
      return { exitCode: await child.exited, stdout, stderr };
    };
    const help = await run("--help");
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("usage: install.sh");
    expect(help.stdout).toContain("MAESTRO_SOURCE_DIR");
    expect(existsSync(join(fixture.home, ".maestro", "source"))).toBe(false);

    const unknown = await run("--bogus");
    expect(unknown.exitCode).toBe(2);
    expect(unknown.stderr).toContain("unknown argument --bogus");
    expect(existsSync(join(fixture.home, ".maestro", "source"))).toBe(false);
  });
});
