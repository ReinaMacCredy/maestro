const LEGACY_MANAGED_SKILL_PREFIX = "maestro:";
const MANAGED_SKILL_DIRECTORY_PREFIX = "maestro%3A";

export function resolveSkillDirectoryName(skillName: string): string {
  return skillName.replaceAll(":", "%3A");
}

export function decodeSkillDirectoryName(directoryName: string): string {
  return directoryName.replaceAll("%3A", ":");
}

export function isManagedSkillDirectoryName(name: string): boolean {
  return name.startsWith(LEGACY_MANAGED_SKILL_PREFIX)
    || name.startsWith(MANAGED_SKILL_DIRECTORY_PREFIX);
}
