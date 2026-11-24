import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { chmod, cp, mkdir, readdir, readFile, readlink, stat, symlink, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { resolveHomeDirectory } from "../src/plugins/home.ts";
import { seatConfigRoot, seatLaunchPath, seatTokenPath, writeSecretFile } from "../src/plugins/slp-seats.ts";
import { enableSlp, hostEnvironment, idFrom, prepareInstallFixture, runCli, runInstalledCliAt, storedSessions, withFixture } from "./helpers.ts";

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

test("439 install warns about a live work holder and still completes", async () => {
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
        "live install work",
      ],
      peer,
    );
    expect(added.exitCode).toBe(0);
    const work = idFrom(added);
    expect((await runCli(fixture, ["work", "start", work], peer)).exitCode).toBe(0);
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], {
      MAESTRO_SESSION_ID: "activation-installer",
      PATH: path,
    });

    expect(installed.exitCode).toBe(0);
    expect(installed.stderr).toContain(
      `[install] 1 live session holds work (repos: ${fixture.repo}); they load the new runtime on their next maestro call`,
    );
  });
});

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
    // room d140 amends d137: Claude Code has no PostCompact event, so the
    // compact count rides the SessionStart hook that is already installed and
    // neither harness gains an event here.
    expect(claudeHooks.hooks.PostCompact).toBeUndefined();
    expect(codexHooks.hooks.PostCompact).toBeUndefined();
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
      expect(stdout).toContain("held work:");
      expect(storedSessions(fixture).find((session) => session.id === sessionId)?.harness).toBe(
        harness,
      );
    }

    // room d137/d140 end to end: the installed Claude adapter forwards the
    // SessionStart source, and source "compact" is the one that records a
    // compaction row. A plain startup records none.
    const compactAdapter = join(fixture.repo, ".claude", "hooks", "maestro-record.ts");
    for (const source of ["compact", "startup"]) {
      const hook = Bun.spawn([process.execPath, compactAdapter], {
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
          session_id: `install-compact-${source}`,
          source,
        }),
      );
      hook.stdin.end();
      expect(await hook.exited).toBe(0);
    }
    const events = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      expect(
        events
          .query<{ count: number }, []>(
            "SELECT COUNT(*) AS count FROM event_log WHERE type = 'session.compact'",
          )
          .get()?.count,
      ).toBe(1);
      expect(
        events
          .query<{ session_id: string | null }, []>(
            "SELECT session_id FROM event_log WHERE type = 'session.compact'",
          )
          .get()?.session_id,
      ).toBe("install-compact-compact");
    } finally {
      events.close();
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
    expect(claudeHooks).toContain('bun \\"${CLAUDE_PROJECT_DIR:-.}/.claude/hooks/maestro-record.ts\\"');
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

test("seat-render: install writes each seat's Claude agent file and Codex session file, is byte-stable, and uninstall removes exactly what it wrote while user files stay untouched (A1, d906)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
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
    expect(installed.stdout).toContain("seat dirs:");
    expect(installed.stdout).not.toContain("retired");

    const leadClaude = await readFile(seatLaunchPath(fixture.home, "claude", "lead"), "utf8");
    expect(leadClaude.startsWith("---\nname: maestro-lead\ndescription: \"fixture lead\"\nmodel: sonnet\neffort: high\npermissionMode: acceptEdits\n---\n\n")).toBe(true);
    expect(leadClaude).toContain("## Shared contract");
    expect(leadClaude.trimEnd().endsWith("Role: fixture lead.")).toBe(true);
    const leadSession = await readFile(seatLaunchPath(fixture.home, "codex", "lead"), "utf8");
    expect(leadSession).toContain('model = "sonnet"\n');
    expect(leadSession).toContain('model_reasoning_effort = "high"\n');
    expect(leadSession).toContain('developer_instructions = """\n');
    expect(leadSession).toContain("Role: fixture lead.");

    // model: default omits the model line on both carriers.
    for (const file of [seatLaunchPath(fixture.home, "claude", "peer"), seatLaunchPath(fixture.home, "codex", "peer")]) {
      expect(await readFile(file, "utf8")).not.toMatch(/^model[ :]/m);
    }
    expect((await readdir(claudeAgents)).filter((entry) => entry.startsWith("maestro-"))).toEqual([]);
    expect((await readdir(codexAgents)).filter((entry) => entry.startsWith("maestro-"))).toEqual([]);

    const seatAgents = ["lead", "peer"].map((seat) => join(seatConfigRoot(fixture.home), seat, "agents"));
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
    expect(written.length).toBe(4);
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

// d906: maestro ships no sub-agent definitions, so install removes the ones an
// earlier install rendered - recognised by the lines the renderer wrote, never
// by the file name alone.
test("retire-agent-files: install removes the maestro-* agent files an earlier install rendered, keeps a hand-written maestro- file without the renderer's lines, reports the count once, and a second install says nothing (d906)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const { path } = await prepareInstallFixture(fixture);
    const claudeAgents = join(fixture.home, ".claude", "agents");
    const codexHome = join(fixture.home, ".codex");
    const codexAgents = join(codexHome, "agents");
    const peerAgents = join(seatConfigRoot(fixture.home), "peer", "agents");
    for (const directory of [claudeAgents, codexAgents, peerAgents]) await mkdir(directory, { recursive: true });
    const rendered = new Map([
      [join(claudeAgents, "maestro-delegate.md"), '---\nname: maestro-delegate\ndescription: "Delegate"\nmodel: opus\n---\n\nRole: Delegate.\n'],
      [join(claudeAgents, "maestro-lead.md"), '---\nname: maestro-lead\ndescription: "old seat file"\n---\n\nRole: Lead.\n'],
      [join(codexAgents, "maestro-delegate.toml"), 'name = "maestro-delegate"\ndescription = "Delegate"\nmodel = "opus"\ndeveloper_instructions = """\nRole: Delegate.\n"""\n'],
      [join(codexAgents, "maestro-peer.toml"), 'name = "maestro-peer"\ndescription = "bounded execution"\ndeveloper_instructions = """\nRole: Peer.\n"""\n'],
      [join(codexHome, "maestro-peer-delegate.config.toml"), 'model = "opus"\ndeveloper_instructions = """\nRole: Delegate.\n"""\n'],
      [join(peerAgents, "maestro-peer-delegate.md"), '---\nname: maestro-peer-delegate\ndescription: "Delegate"\n---\n\nRole: Delegate.\n'],
    ]);
    const handWritten = new Map([
      [join(claudeAgents, "maestro-mine.md"), "---\nname: my-own-agent\n---\nhand written\n"],
      [join(codexAgents, "maestro-mine.toml"), 'name = "Mine"\ndeveloper_instructions = "mine"\n'],
      [join(codexHome, "maestro-mine.config.toml"), 'model = "gpt-5"\nmodel_provider = "mine"\n'],
    ]);
    for (const [file, content] of [...rendered, ...handWritten]) await writeFile(file, content);

    const first = await runCli(fixture, ["install"], { PATH: path });
    expect(first.exitCode).toBe(0);
    expect(first.stdout.match(/retired \d+ maestro agent files/g)).toEqual(["retired 6 maestro agent files"]);
    for (const file of rendered.keys()) expect({ file, exists: existsSync(file) }).toEqual({ file, exists: false });
    for (const [file, content] of handWritten) expect(await readFile(file, "utf8")).toBe(content);
    for (const seat of ["lead", "peer"] as const) {
      for (const harness of ["claude", "codex"] as const) {
        expect(existsSync(seatLaunchPath(fixture.home, harness, seat))).toBe(true);
      }
    }

    const second = await runCli(fixture, ["install"], { PATH: path });
    expect(second.exitCode).toBe(0);
    expect(second.stdout).not.toContain("retired");
    for (const [file, content] of handWritten) expect(await readFile(file, "utf8")).toBe(content);
  });
}, 30_000);

// seat-config-dirs R4 (d848, d849): a seat dir's skills/ is the seat's skills:
// list, resolved in ~/maestro/skills then ~/.claude/skills; a stale link goes;
// projects and plugins reach ~/.claude; .claude.json marks onboarding done with
// no MCP servers.
test("pstack-port: an owner seat file that still lists a retired skill installs, and the seat links skip that skill (d863)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const { path } = await prepareInstallFixture(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: opus\nskills: [maestro-work, maestro-explore, maestro-diagnose]\n---\nRole: owner lead.\n",
    );

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const leadLinks = (await readdir(join(seatConfigRoot(fixture.home), "lead", "skills"))).sort();
    expect(leadLinks).toContain("maestro-work");
    expect(leadLinks).not.toContain("maestro-explore");
    expect(leadLinks).not.toContain("maestro-diagnose");
  });
});

test("seat-dirs-links: install renders skills/ from the seat's skills: list, removes a stale link, links projects and plugins to ~/.claude and writes .claude.json with mcpServers {} (R4)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const { path } = await prepareInstallFixture(fixture);
    const ownerSkill = join(fixture.home, ".claude", "skills", "owner-skill");
    await mkdir(ownerSkill, { recursive: true });
    await writeFile(join(ownerSkill, "SKILL.md"), "---\nname: owner-skill\n---\nowner\n");
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "peer.md"),
      "---\nharness: codex\nmodel: default\nskills: [maestro-work, maestro-verify, maestro-bundle, owner-skill]\n---\n",
    );
    const peerDir = join(seatConfigRoot(fixture.home), "peer");
    await mkdir(join(peerDir, "skills"), { recursive: true });
    await symlink(join(fixture.home, "nowhere"), join(peerDir, "skills", "stale-skill"));

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const links = (await readdir(join(peerDir, "skills"))).sort();
    expect(links).toEqual(["maestro-bundle", "maestro-verify", "maestro-work", "owner-skill"]);
    expect(await readlink(join(peerDir, "skills", "maestro-work"))).toBe(join(fixture.home, "maestro", "skills", "maestro-work"));
    expect(await readlink(join(peerDir, "skills", "owner-skill"))).toBe(ownerSkill);
    expect(existsSync(join(peerDir, "skills", "maestro-work", "SKILL.md"))).toBe(true);
    const leadLinks = (await readdir(join(seatConfigRoot(fixture.home), "lead", "skills"))).sort();
    expect(leadLinks).toEqual(["maestro-council", "maestro-design", "maestro-work"]);

    for (const seat of ["lead", "peer"]) {
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
    await enableSlp(fixture);
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
    for (const seat of ["lead", "peer"]) {
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

// seat-config-dirs R15 (d855 rule 4): under umask 022 a default write lands
// 0644 and a default mkdir 0755 before any chmod (SEC-3), and the seat
// .claude.json had no chmod at all (SEC-5); every seat secret file is created
// 0600 and every seat dir 0700, and a pre-existing file is brought to 0600.
test("seat-dirs-modes: under umask 022 install --seat-token leaves the token file, every seat settings.json and every seat .claude.json at 0600 and the seat root and dirs at 0700, a pre-existing 0644 .claude.json and settings.json included (R15, d855)", async () => {
  const previous = process.umask(0o022);
  try {
    await withFixture(async (fixture) => {
      await enableSlp(fixture);
      const { path } = await prepareInstallFixture(fixture);
      const leadDir = join(seatConfigRoot(fixture.home), "lead");
      await mkdir(leadDir, { recursive: true });
      await writeFile(join(leadDir, ".claude.json"), '{"hasCompletedOnboarding":true,"mcpServers":{},"numStartups":1}\n');
      await writeFile(join(leadDir, "settings.json"), "{}\n");
      for (const file of [".claude.json", "settings.json"]) await chmod(join(leadDir, file), 0o644);
      const mode = async (target: string) => (await stat(target)).mode & 0o777;

      const installed = await runCli(fixture, ["install", "--seat-token"], { PATH: path }, "sk-ant-oat01-R15-fixture\n");
      expect(installed.exitCode).toBe(0);
      expect(await mode(seatTokenPath(fixture.home))).toBe(0o600);
      expect(await mode(seatConfigRoot(fixture.home))).toBe(0o700);
      for (const seat of ["lead", "peer"]) {
        const directory = join(seatConfigRoot(fixture.home), seat);
        expect({ seat, mode: await mode(directory) }).toEqual({ seat, mode: 0o700 });
        expect({ seat, file: "settings.json", mode: await mode(join(directory, "settings.json")) }).toEqual({ seat, file: "settings.json", mode: 0o600 });
        expect({ seat, file: ".claude.json", mode: await mode(join(directory, ".claude.json")) }).toEqual({ seat, file: ".claude.json", mode: 0o600 });
      }
      expect(JSON.parse(await readFile(join(leadDir, ".claude.json"), "utf8"))).toEqual({ hasCompletedOnboarding: true, mcpServers: {}, numStartups: 1 });
    });
  } finally {
    process.umask(previous);
  }
}, 60_000);

// seat-config-dirs R17 (d855 rule 4, mutant M4): R15 observes only the end
// state, so a write without { mode: 0o600 } followed by a chmod still passed.
// The one helper behind the token file, seat settings.json and seat
// .claude.json creates a fresh file at 0600 and never chmods afterwards, so
// dropping the mode option goes red here.
test("seat-dirs-secret-file: under umask 022 writeSecretFile creates a fresh file at 0600 with no chmod after, and brings an existing 0644 file to 0600 before rewriting it (R17, d855)", async () => {
  const previous = process.umask(0o022);
  try {
    await withFixture(async (fixture) => {
      const mode = async (target: string) => (await stat(target)).mode & 0o777;
      const fresh = join(fixture.home, "fresh-secret");
      await writeSecretFile(fresh, "one\n");
      expect(await mode(fresh)).toBe(0o600);
      expect(await readFile(fresh, "utf8")).toBe("one\n");

      const existing = join(fixture.home, "existing-secret");
      await writeFile(existing, "old\n");
      await chmod(existing, 0o644);
      await writeSecretFile(existing, "new\n");
      expect(await mode(existing)).toBe(0o600);
      expect(await readFile(existing, "utf8")).toBe("new\n");

      await chmod(existing, 0o644);
      await writeSecretFile(existing, "new\n");
      expect(await mode(existing)).toBe(0o600);
    });
  } finally {
    process.umask(previous);
  }
});
