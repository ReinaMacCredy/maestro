import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { chmod, cp, mkdir, readdir, readFile, readlink, stat, symlink, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { resolveHomeDirectory } from "../src/plugins/home.ts";
import { renderedProfilePath, seatConfigRoot, seatTokenPath } from "../src/plugins/profiles.ts";
import { hostEnvironment, idFrom, prepareInstallFixture, runCli, runInstalledCliAt, withFixture } from "./helpers.ts";

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
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-slp-watch")).exists()).toBe(false);
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-team-sensor")).exists()).toBe(false);
    // Hub d97/d98: a stale sentinel shim from an earlier install is removed.
    expect(await Bun.file(join(fixture.home, ".local", "bin", "maestro-slp-observe")).exists()).toBe(false);
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
    expect(await Bun.file(join(localBin, "maestro-slp-watch")).exists()).toBe(false);

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
        env: { ...hostEnvironment(), HOME: fixture.home, PATH: path },
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
          ...hostEnvironment(),
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
          ...hostEnvironment(),
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
      env: { ...hostEnvironment(), HOME: fixture.home, PATH: `${shims}:${process.env.PATH ?? ""}` },
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
        env: { ...hostEnvironment(), HOME: fixture.home },
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

test("profile-render: install renders the three carriers, is byte-stable, and uninstall removes exactly what it wrote while user files stay untouched (red 2, A1)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const claudeAgents = join(fixture.home, ".claude", "agents");
    const codexHome = join(fixture.home, ".codex");
    const codexAgents = join(codexHome, "agents");
    await mkdir(claudeAgents, { recursive: true });
    await mkdir(codexAgents, { recursive: true });
    const userFiles = new Map([
      [join(codexHome, "steyg.config.toml"), 'model = "claude-opus-5"\nmodel_provider = "steyg"\n'],
      [join(codexAgents, "reviewer.toml"), 'name = "Reviewer"\ndeveloper_instructions = "review"\n'],
      [join(claudeAgents, "custom.md"), "---\nname: custom\n---\nhand written\n"],
    ]);
    for (const [file, content] of userFiles) await writeFile(file, content);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: sonnet\neffort: high\npermission: acceptEdits\ndescription: fixture lead\n---\nRole: fixture lead.\n",
    );

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).toContain("profiles rendered:");

    // d845: seat and peer-* renders live in the seat dirs, bare nodes stay here.
    const leadClaude = await readFile(renderedProfilePath(fixture.home, "claude", "lead"), "utf8");
    expect(leadClaude.startsWith("---\nname: maestro-lead\ndescription: \"fixture lead\"\nmodel: sonnet\neffort: high\npermissionMode: acceptEdits\n---\n\n")).toBe(true);
    expect(leadClaude).toContain("## Shared contract");
    expect(leadClaude.trimEnd().endsWith("Role: fixture lead.")).toBe(true);
    const leadSession = await readFile(join(codexHome, "maestro-lead.config.toml"), "utf8");
    expect(leadSession).toContain('model = "sonnet"\n');
    expect(leadSession).toContain('model_reasoning_effort = "high"\n');
    expect(leadSession).toContain('developer_instructions = """\n');
    expect(leadSession).toContain("Role: fixture lead.");
    expect(leadSession).not.toContain("sandbox_mode");
    const leadAgent = await readFile(join(codexAgents, "maestro-lead.toml"), "utf8");
    expect(leadAgent).toContain('name = "maestro-lead"\n');
    expect(leadAgent).toContain('model = "sonnet"\n');
    expect(leadAgent).toContain('model_reasoning_effort = "high"\n');
    expect(leadAgent).toContain("Role: fixture lead.");

    // d93: sandbox renders into the sub-agent file only.
    const verifierAgent = await readFile(join(codexAgents, "maestro-verifier.toml"), "utf8");
    expect(verifierAgent).toContain('sandbox_mode = "read-only"\n');
    expect(await readFile(join(codexHome, "maestro-verifier.config.toml"), "utf8")).not.toContain("sandbox_mode");

    // graph-engine red 14 (d83, d100): every shipped node preset renders into
    // both harness agent dirs with the same instruction body.
    for (const node of [
      "classifier",
      "reviewer-simplify",
      "reviewer-correctness",
      "reviewer-regression",
      "reviewer-contracts",
      "reviewer-security",
      "refuter",
      "fixer",
      "synthesizer",
    ]) {
      const claude = await readFile(join(claudeAgents, `maestro-${node}.md`), "utf8");
      const codex = await readFile(join(codexAgents, `maestro-${node}.toml`), "utf8");
      const body = claude.slice(claude.indexOf("\n---\n\n") + "\n---\n\n".length).trimEnd();
      expect({ node, starts: claude.startsWith(`---\nname: maestro-${node}\n`) }).toEqual({ node, starts: true });
      expect({ node, starts: body.startsWith("Role: ") }).toEqual({ node, starts: true });
      expect({ node, codexHasBody: codex.includes(body) }).toEqual({ node, codexHasBody: true });
      expect(codex).toContain(`name = "maestro-${node}"\n`);
      expect(await Bun.file(renderedProfilePath(fixture.home, "claude", `peer-${node}`)).exists()).toBe(true);
    }

    // model: default omits the model line on all three carriers.
    for (const file of [
      renderedProfilePath(fixture.home, "claude", "peer"),
      join(codexHome, "maestro-peer.config.toml"),
      join(codexAgents, "maestro-peer.toml"),
    ]) {
      expect(await readFile(file, "utf8")).not.toMatch(/^model[ :]/m);
    }

    const seatAgents = ["lead", "peer", "team-supervisor"].map((seat) => join(seatConfigRoot(fixture.home), seat, "agents"));
    const snapshot = async () => {
      const files = new Map<string, string>();
      for (const directory of [claudeAgents, ...seatAgents, codexHome, codexAgents]) {
        if (!existsSync(directory)) continue;
        for (const entry of (await readdir(directory)).sort()) {
          const file = join(directory, entry);
          if ((await stat(file)).isFile()) files.set(file, await readFile(file, "utf8"));
        }
      }
      return files;
    };
    const afterFirst = await snapshot();
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await snapshot()).toEqual(afterFirst);

    const uninstalled = await runCli(fixture, ["uninstall"], { PATH: path });
    expect(uninstalled.exitCode).toBe(0);
    const afterUninstall = await snapshot();
    const written = [...afterFirst.keys()].filter((file) => /\/maestro-[^/]+$/.test(file));
    expect(written.length).toBeGreaterThan(0);
    for (const file of written) expect(afterUninstall.has(file)).toBe(false);
    for (const [file, content] of afterFirst) {
      if (!written.includes(file)) expect(afterUninstall.get(file)).toBe(content);
    }
    for (const [file, content] of userFiles) {
      expect(afterFirst.get(file)).toBe(content);
      expect(afterUninstall.get(file)).toBe(content);
    }
  });
}, 30_000);

// seat-config-dirs R4 (d848, d849): the peer dir's skills/ is the union across
// every profile rendered into it, resolved in ~/maestro/skills then
// ~/.claude/skills; a stale link goes; projects and plugins reach ~/.claude;
// .claude.json marks onboarding done with no MCP servers.
test("seat-dirs-links: install renders skills/ as the union across the dir's profiles, removes a stale link, links projects and plugins to ~/.claude and writes .claude.json with mcpServers {} (R4)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const ownerSkill = join(fixture.home, ".claude", "skills", "owner-skill");
    await mkdir(ownerSkill, { recursive: true });
    await writeFile(join(ownerSkill, "SKILL.md"), "---\nname: owner-skill\n---\nowner\n");
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "refuter.md"),
      "---\nharness: claude\nmodel: default\nskills: [maestro-bundle, owner-skill]\n---\nRole: Refuter.\n",
    );
    const peerDir = join(seatConfigRoot(fixture.home), "peer");
    await mkdir(join(peerDir, "skills"), { recursive: true });
    await symlink(join(fixture.home, "nowhere"), join(peerDir, "skills", "stale-skill"));

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const links = (await readdir(join(peerDir, "skills"))).sort();
    expect(links).toEqual(["maestro-bundle", "maestro-diagnose", "maestro-explore", "maestro-verify", "maestro-work", "owner-skill"]);
    expect(await readlink(join(peerDir, "skills", "maestro-work"))).toBe(join(fixture.home, "maestro", "skills", "maestro-work"));
    expect(await readlink(join(peerDir, "skills", "owner-skill"))).toBe(ownerSkill);
    expect(existsSync(join(peerDir, "skills", "maestro-work", "SKILL.md"))).toBe(true);
    const leadLinks = (await readdir(join(seatConfigRoot(fixture.home), "lead", "skills"))).sort();
    expect(leadLinks).toEqual(["maestro-council", "maestro-design", "maestro-diagnose", "maestro-explore", "maestro-graph", "maestro-work"]);
    expect(await readdir(join(seatConfigRoot(fixture.home), "team-supervisor", "skills"))).toEqual(["maestro-work"]);

    for (const seat of ["lead", "peer", "team-supervisor"]) {
      const directory = join(seatConfigRoot(fixture.home), seat);
      expect(await readlink(join(directory, "projects"))).toBe(join(fixture.home, ".claude", "projects"));
      expect(await readlink(join(directory, "plugins"))).toBe(join(fixture.home, ".claude", "plugins"));
      expect(JSON.parse(await readFile(join(directory, ".claude.json"), "utf8"))).toEqual({
        hasCompletedOnboarding: true,
        mcpServers: {},
      });
    }
    // Claude Code owns .claude.json once it exists: a second install leaves it alone.
    await writeFile(join(peerDir, ".claude.json"), JSON.stringify({ hasCompletedOnboarding: true, mcpServers: {}, projects: { x: 1 } }));
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(JSON.parse(await readFile(join(peerDir, ".claude.json"), "utf8"))).toEqual({ hasCompletedOnboarding: true, mcpServers: {}, projects: { x: 1 } });
  });
}, 30_000);

// seat-config-dirs R5 (d846, A1, A2): the token arrives on stdin and lands only
// in the 0600 token file and each seat settings env; install output never
// carries it; doctor reports it; the owner's user settings stay byte for byte.
test("seat-dirs-token: install --seat-token from stdin writes the 0600 token file, never prints the token, doctor reports missing then present, and ~/.claude/settings.json plus ~/.claude.json are byte-identical across install (R5)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const userSettings = join(fixture.home, ".claude", "settings.json");
    const userClaudeJson = join(fixture.home, ".claude.json");
    await mkdir(join(fixture.home, ".claude"), { recursive: true });
    const userSettingsText = JSON.stringify({
      model: "opus",
      hooks: {
        SessionStart: [
          { matcher: "*", hooks: [{ type: "command", command: `bash '${join(fixture.home, ".claude", "hooks", "herdr-agent-state.sh")}' session`, timeout: 10 }] },
          { hooks: [{ type: "command", command: "echo other" }] },
        ],
      },
    }, null, 2);
    const userClaudeJsonText = '{"hasCompletedOnboarding":true,"numStartups":42,"projects":{"/x":{"allowedTools":[]}}}';
    await writeFile(userSettings, userSettingsText);
    await writeFile(userClaudeJson, userClaudeJsonText);
    const token = "sk-ant-oat01-R5-secret-token-value";

    const withoutToken = await runCli(fixture, ["install"], { PATH: path });
    expect(withoutToken.exitCode).toBe(0);
    expect(withoutToken.stdout).toContain("--seat-token");
    expect(withoutToken.stdout).toContain(seatTokenPath(fixture.home));
    const missing = await runCli(fixture, ["doctor"], { PATH: path });
    expect(missing.exitCode).toBe(0);
    expect(missing.stdout).toContain("seat token: missing");
    const peerSettingsBefore = JSON.parse(await readFile(join(seatConfigRoot(fixture.home), "peer", "settings.json"), "utf8")) as { env: Record<string, string> };
    expect("CLAUDE_CODE_OAUTH_TOKEN" in peerSettingsBefore.env).toBe(false);

    const empty = await runCli(fixture, ["install", "--seat-token"], { PATH: path }, "  \n");
    expect(empty.exitCode).toBe(1);
    expect(JSON.parse(empty.stderr).error.code).toBe("SEAT_TOKEN_EMPTY");
    expect(existsSync(seatTokenPath(fixture.home))).toBe(false);

    const installed = await runCli(fixture, ["install", "--seat-token"], { PATH: path }, `${token}\n`);
    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).not.toContain(token);
    expect(installed.stderr).not.toContain(token);
    expect(installed.stdout).not.toContain("--seat-token");
    expect(await readFile(seatTokenPath(fixture.home), "utf8")).toBe(token);
    expect((await stat(seatTokenPath(fixture.home))).mode & 0o777).toBe(0o600);
    for (const seat of ["lead", "peer", "team-supervisor"]) {
      const settings = JSON.parse(await readFile(join(seatConfigRoot(fixture.home), seat, "settings.json"), "utf8")) as { env: Record<string, string>; hooks: unknown };
      expect({ seat, token: settings.env.CLAUDE_CODE_OAUTH_TOKEN }).toEqual({ seat, token });
      // A6: the owner's own herdr hook group is the one carried, the other group is not.
      expect(settings.hooks).toEqual({
        SessionStart: [{ matcher: "*", hooks: [{ type: "command", command: `bash '${join(fixture.home, ".claude", "hooks", "herdr-agent-state.sh")}' session`, timeout: 10 }] }],
      });
    }
    const present = await runCli(fixture, ["doctor"], { PATH: path });
    expect(present.exitCode).toBe(0);
    expect(present.stdout).toContain("seat token: present");
    expect(present.stdout).not.toContain(token);

    // A plain install keeps the token and re-renders it.
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await readFile(seatTokenPath(fixture.home), "utf8")).toBe(token);
    expect(await readFile(join(seatConfigRoot(fixture.home), "lead", "settings.json"), "utf8")).toContain(token);

    // A1: nothing outside maestro-managed paths changed.
    expect(await readFile(userSettings, "utf8")).toBe(userSettingsText);
    expect(await readFile(userClaudeJson, "utf8")).toBe(userClaudeJsonText);
  });
}, 60_000);
