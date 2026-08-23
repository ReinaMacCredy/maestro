import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("A5 install preserves rollback before writing the shim and wires temp-only hooks", async () => {
  await withFixture(async (fixture) => {
    const localBin = join(fixture.home, ".local", "bin");
    const shim = join(localBin, "maestro");
    const legacy = join(localBin, "maestro-legacy");
    await mkdir(localBin, { recursive: true });
    const legacySource = "#!/bin/sh\necho legacy-maestro\n";
    await writeFile(shim, legacySource);
    await chmod(shim, 0o755);
    const path = `${localBin}:${process.env.PATH ?? ""}`;

    const installed = await runCli(fixture, ["install"], { PATH: path });

    expect(installed.exitCode).toBe(0);
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

    const hookPath = join(fixture.repo, ".maestro", "hooks", "record.ts");
    const hook = Bun.spawn([process.execPath, hookPath], {
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
        session_id: "install-hook-session",
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
  });
});
