import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

let tmpDir: string;
let repoDir: string;
let homeDir: string;
let env: Record<string, string>;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-providers-e2e-"));
  repoDir = join(tmpDir, "repo");
  homeDir = join(tmpDir, "home");
  await mkdir(repoDir, { recursive: true });
  await mkdir(homeDir, { recursive: true });
  await initGitRepo(repoDir);
  await runCommand(["git", "config", "user.name", "Test User"], repoDir);
  await runCommand(["git", "config", "user.email", "test@example.com"], repoDir);
  await writeFile(join(repoDir, "README.md"), "# temp\n");
  await runCommand(["git", "add", "README.md"], repoDir);
  await runCommand(["git", "commit", "-m", "init"], repoDir);
  env = {
    HOME: homeDir,
    USERPROFILE: homeDir,
    MAESTRO_HOME: join(homeDir, ".maestro"),
    HERMES_HOME: join(homeDir, ".hermes"),
    CODEX_HOME: join(homeDir, ".codex"),
    CLAUDECODE: "",
    CODEX_THREAD_ID: "",
    MAESTRO_AGENT: "",
    MAESTRO_SESSION_ID: "",
  };
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled provider and skill commands", () => {
  it("lists providers as structured JSON", async () => {
    const result = await runCompiled(["providers", "list", "--json"], repoDir, { env });

    expect(result.exitCode).toBe(0);
    const providers = expectJson<Array<{ id: string; runtime: boolean; skillTarget: boolean }>>(result);
    expect(providers.map((provider) => provider.id).sort()).toEqual([
      "agentskills",
      "claude",
      "codex",
      "hermes",
    ]);
    expect(providers.find((provider) => provider.id === "agentskills")).toMatchObject({
      runtime: false,
      skillTarget: true,
    });
  });

  it("installs a local skill into managed storage and syncs selected targets", async () => {
    const source = join(tmpDir, "demo-skill");
    await writeSkill(source, "demo-skill", "Demo skill");

    const install = await runCompiled(
      ["skills", "install", source, "--scope", "user", "--targets", "hermes,agentskills", "--json"],
      repoDir,
      { env },
    );

    expect(install.exitCode).toBe(0);
    const payload = expectJson<Array<{ name: string; installedTargets: string[] }>>(install);
    expect(payload[0]).toMatchObject({
      name: "demo-skill",
      installedTargets: ["hermes", "agentskills"],
    });
    expect(existsSync(join(homeDir, ".maestro", "external-skills", "demo-skill", "SKILL.md"))).toBe(true);
    expect(existsSync(join(homeDir, ".hermes", "skills", "maestro", "demo-skill", "SKILL.md"))).toBe(true);
    expect(existsSync(join(homeDir, ".agents", "skills", "demo-skill", "SKILL.md"))).toBe(true);

    const listed = await runCompiled(["skills", "list", "--scope", "all", "--json"], repoDir, { env });
    expect(listed.exitCode).toBe(0);
    const listPayload = expectJson<{ skills: Array<{ name: string }>; diagnostics: unknown[] }>(listed);
    expect(listPayload.skills.some((skill) => skill.name === "demo-skill")).toBe(true);
    expect(Array.isArray(listPayload.diagnostics)).toBe(true);
  });

  it("syncs bundled skills only to the selected provider targets", async () => {
    const synced = await runCompiled(["skills", "sync", "--targets", "agentskills", "--json"], repoDir, { env });

    expect(synced.exitCode).toBe(0);
    const payload = expectJson<{ bundled: Array<{ agent: string }> }>(synced);
    expect(payload.bundled.map((entry) => entry.agent)).toEqual(["AgentSkills"]);
    expect(existsSync(join(homeDir, ".agents", "skills", "maestro-task", "SKILL.md"))).toBe(true);
    expect(existsSync(join(homeDir, ".hermes", "skills", "maestro", "maestro-task", "SKILL.md"))).toBe(false);
  });

});

async function writeSkill(dir: string, name: string, description: string): Promise<void> {
  await mkdir(dir, { recursive: true });
  await writeFile(
    join(dir, "SKILL.md"),
    ["---", `name: ${name}`, `description: ${description}`, "---", "# Skill"].join("\n"),
  );
}
