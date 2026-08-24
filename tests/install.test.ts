import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

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
    expect(await readFile(join(fixture.repo, "AGENTS.md"), "utf8")).toContain("maestro status");
    expect(await readFile(join(fixture.repo, "CLAUDE.md"), "utf8")).toContain("maestro ready");

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

test("29 install writes portable hook files without machine-absolute paths", async () => {
  await withFixture(async (fixture) => {
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

test("45 install mirrors name the manual hookless SessionStart bootstrap without Cursor wiring", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });
    const agents = await readFile(join(fixture.repo, "AGENTS.md"), "utf8");
    const claude = await readFile(join(fixture.repo, "CLAUDE.md"), "utf8");
    const bootstrap =
      "If no harness hook fired, run `maestro hook record --event SessionStart` and read the brief from stdout.";

    expect(installed.exitCode).toBe(0);
    expect(agents).toContain(bootstrap);
    expect(claude).toContain(bootstrap);
    expect(existsSync(join(fixture.repo, ".cursor"))).toBe(false);
  });
});
