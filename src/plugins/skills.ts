import { existsSync } from "node:fs";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

export const skillNames = [
  "maestro-bundle",
  "maestro-design",
  "maestro-work",
  "maestro-verify",
] as const;

const stampPattern = /<!-- maestro-skill-version: ([0-9a-f]{40}|dev) -->/;

export interface SkillSync {
  current: string[];
  skipped: string[];
  written: string[];
}

// The shared agents layer is dotfiles-versioned and holds user-owned skills;
// only dirs whose SKILL.md carries our stamp are ever rewritten.
export async function materializeSkills(home: string, commit: string): Promise<SkillSync> {
  const sourceRoot = join(import.meta.dir, "skills");
  const targetRoot = join(home, ".agents", "skills");
  const sync: SkillSync = { current: [], skipped: [], written: [] };
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
  return sync;
}

export function formatSkillSync(sync: SkillSync): string {
  const parts: string[] = [];
  if (sync.written.length > 0) parts.push(`skills wrote: ${sync.written.join(", ")}`);
  if (sync.current.length > 0) parts.push(`skills current: ${sync.current.join(", ")}`);
  for (const name of sync.skipped) {
    parts.push(`skills skipped: ${name} (unmanaged SKILL.md; remove it or manage it yourself)`);
  }
  return parts.join("\n");
}
