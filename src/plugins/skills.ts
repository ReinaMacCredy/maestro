import { existsSync } from "node:fs";
import { cp, lstat, mkdir, readFile, readlink, rm, symlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

export const skillNames = [
  "maestro-bundle",
  "maestro-design",
  "maestro-work",
  "maestro-verify",
] as const;

const stampPattern = /<!-- maestro-skill-version: ([0-9a-f]{40}|dev) -->/;

export interface SkillSync {
  current: string[];
  legacyRemoved: string[];
  legacySkipped: string[];
  linked: string[];
  linkSkipped: string[];
  skipped: string[];
  written: string[];
}

async function managedSkill(directory: string): Promise<boolean> {
  const skill = join(directory, "SKILL.md");
  if (!existsSync(skill)) return false;
  return stampPattern.test(await readFile(skill, "utf8"));
}

async function linkSkill(
  link: string,
  target: string,
  sync: SkillSync,
  name: string,
): Promise<void> {
  try {
    const entry = await lstat(link);
    if (entry.isSymbolicLink()) {
      const current = resolve(dirname(link), await readlink(link));
      if (current === target) return;
      await rm(link, { force: true });
    } else if (await managedSkill(link)) {
      await rm(link, { recursive: true, force: true });
    } else {
      sync.linkSkipped.push(name);
      return;
    }
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") throw error;
  }
  await mkdir(dirname(link), { recursive: true });
  await symlink(target, link);
  sync.linked.push(name);
}

// The shared agents layer is dotfiles-versioned and holds user-owned skills;
// only dirs whose SKILL.md carries our stamp are ever rewritten.
export async function materializeSkills(home: string, commit: string): Promise<SkillSync> {
  const sourceRoot = join(import.meta.dir, "skills");
  const targetRoot = join(home, "maestro", "skills");
  const sync: SkillSync = {
    current: [],
    legacyRemoved: [],
    legacySkipped: [],
    linked: [],
    linkSkipped: [],
    skipped: [],
    written: [],
  };
  for (const name of skillNames) {
    const target = join(targetRoot, name);
    const targetSkill = join(target, "SKILL.md");
    if (existsSync(targetSkill)) {
      const existing = await readFile(targetSkill, "utf8");
      const stamp = stampPattern.exec(existing);
      if (!stamp) {
        sync.skipped.push(name);
        continue;
      }
      if (stamp[1] === commit) {
        sync.current.push(name);
        continue;
      }
    }
    await rm(target, { recursive: true, force: true });
    await mkdir(targetRoot, { recursive: true });
    await cp(join(sourceRoot, name), target, { recursive: true });
    const skill = await readFile(targetSkill, "utf8");
    await writeFile(targetSkill, skill.replace(stampPattern, `<!-- maestro-skill-version: ${commit} -->`));
    sync.written.push(name);
  }
  for (const name of skillNames) {
    const target = join(targetRoot, name);
    await linkSkill(join(home, ".claude", "skills", name), target, sync, name);
    const legacy = join(home, ".agents", "skills", name);
    if (!existsSync(legacy)) continue;
    if (await managedSkill(legacy)) {
      await rm(legacy, { recursive: true, force: true });
      sync.legacyRemoved.push(name);
    } else {
      sync.legacySkipped.push(name);
    }
  }
  return sync;
}

export function formatSkillSync(sync: SkillSync): string {
  const parts: string[] = [];
  if (sync.written.length > 0) parts.push(`skills wrote: ${sync.written.join(", ")}`);
  if (sync.current.length > 0) parts.push(`skills current: ${sync.current.join(", ")}`);
  if (sync.linked.length > 0) parts.push(`skills linked for Claude: ${sync.linked.join(", ")}`);
  if (sync.legacyRemoved.length > 0) {
    parts.push(`legacy skills removed from ~/.agents: ${sync.legacyRemoved.join(", ")}`);
  }
  for (const name of sync.skipped) {
    parts.push(`skills skipped: ${name} (unmanaged SKILL.md; remove it or manage it yourself)`);
  }
  for (const name of sync.linkSkipped) {
    parts.push(`skill link skipped: ${name} (unmanaged Claude skill; remove it or manage it yourself)`);
  }
  for (const name of sync.legacySkipped) {
    parts.push(`legacy skill preserved: ${name} (unmanaged ~/.agents skill)`);
  }
  return parts.join("\n");
}
