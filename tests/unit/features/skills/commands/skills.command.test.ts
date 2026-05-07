import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  discoverSkills,
  installSkillSource,
  parseSkillMarkdown,
  removeManagedSkill,
} from "@/features/skills";

describe("skills command helpers", () => {
  let tmpDir: string;
  let cwd: string;
  let homeDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-skills-"));
    cwd = join(tmpDir, "repo");
    homeDir = join(tmpDir, "home");
    await mkdir(cwd, { recursive: true });
    await mkdir(homeDir, { recursive: true });
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("parses AgentSkills frontmatter and preserves unknown metadata", () => {
    const parsed = parseSkillMarkdown(
      [
        "---",
        "name: example-skill",
        "description: Use this for examples.",
        "license: MIT",
        "tags:",
        "  - test",
        "---",
        "# Body",
      ].join("\n"),
      "example-skill",
    );

    expect(parsed.diagnostics).toEqual([]);
    expect(parsed.skill?.name).toBe("example-skill");
    expect(parsed.skill?.description).toBe("Use this for examples.");
    expect(parsed.skill?.metadata).toEqual({ license: "MIT", tags: ["test"] });
    expect(parsed.skill?.body).toContain("# Body");
  });

  it("reports malformed YAML and missing required fields", () => {
    expect(parseSkillMarkdown("---\nname: [\n---\n# Body", "bad").diagnostics[0]?.level).toBe("error");

    const missing = parseSkillMarkdown("---\nname: missing-description\n---\n# Body", "missing-description");
    expect(missing.skill).toBeUndefined();
    expect(missing.diagnostics.map((d) => d.message)).toContain("SKILL.md frontmatter requires description");
  });

  it("uses deterministic precedence and reports collisions", async () => {
    await writeSkill(join(cwd, ".maestro", "skills", "alpha"), {
      name: "alpha",
      description: "Project Maestro skill",
    });
    await writeSkill(join(cwd, ".agents", "skills", "alpha"), {
      name: "alpha",
      description: "Project AgentSkills skill",
    });

    const result = await discoverSkills({ cwd, homeDir, scope: "all" });

    expect(result.skills).toHaveLength(1);
    expect(result.skills[0]?.description).toBe("Project Maestro skill");
    expect(result.diagnostics.some((d) => d.message.includes("is shadowed by"))).toBe(true);
  });

  it("installs a local skill, writes a manifest, syncs target links, and removes only managed entries", async () => {
    const source = join(tmpDir, "source-skill");
    await writeSkill(source, {
      name: "local-skill",
      description: "Local install",
    });

    const installed = await installSkillSource({
      source,
      scope: "user",
      targets: ["hermes", "agentskills"],
      cwd,
      homeDir,
    });

    expect(installed).toHaveLength(1);
    expect([...(installed[0]?.installedTargets ?? [])].sort()).toEqual(["agentskills", "hermes"]);
    expect(existsSync(join(homeDir, ".maestro", "external-skills", "local-skill", "SKILL.md"))).toBe(true);
    expect(existsSync(join(homeDir, ".hermes", "skills", "maestro", "local-skill", "SKILL.md"))).toBe(true);
    expect(existsSync(join(homeDir, ".agents", "skills", "local-skill", "SKILL.md"))).toBe(true);

    const manifest = JSON.parse(
      await readFile(join(homeDir, ".maestro", "external-skills", "local-skill", ".maestro-external-skill.json"), "utf8"),
    );
    expect(manifest.managedBy).toBe("maestro");
    expect(manifest.source).toBe(source);
    expect(manifest.fileHashes["SKILL.md"]).toMatch(/^[0-9a-f]{64}$/);
    expect(manifest.installedTargetRoots).toContain(join(homeDir, ".hermes", "skills", "maestro"));
    expect(manifest.installedTargetRoots).toContain(join(homeDir, ".agents", "skills"));

    const removed = await removeManagedSkill({ name: "local-skill", scope: "user", cwd, homeDir });

    expect(removed.removed).toBe(true);
    expect([...removed.removedTargets].sort()).toEqual(["agentskills", "hermes"]);
    expect(existsSync(join(homeDir, ".maestro", "external-skills", "local-skill"))).toBe(false);
    expect(existsSync(join(homeDir, ".hermes", "skills", "maestro", "local-skill"))).toBe(false);
    expect(existsSync(join(homeDir, ".agents", "skills", "local-skill"))).toBe(false);
  });

  it("resolves relative local install sources against the supplied cwd", async () => {
    await writeSkill(join(cwd, "relative-skill"), {
      name: "relative-skill",
      description: "Relative install",
    });

    const installed = await installSkillSource({
      source: "relative-skill",
      scope: "user",
      targets: ["agentskills"],
      cwd,
      homeDir,
    });

    expect(installed[0]?.name).toBe("relative-skill");
    expect(existsSync(join(homeDir, ".agents", "skills", "relative-skill", "SKILL.md"))).toBe(true);
  });
});

async function writeSkill(
  dir: string,
  frontmatter: { readonly name: string; readonly description: string },
): Promise<void> {
  await mkdir(dir, { recursive: true });
  await writeFile(
    join(dir, "SKILL.md"),
    [
      "---",
      `name: ${frontmatter.name}`,
      `description: ${frontmatter.description}`,
      "---",
      "# Skill",
    ].join("\n"),
  );
}
