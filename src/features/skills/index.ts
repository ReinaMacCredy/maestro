export { registerSkillsCommand } from "./commands/skills.command.js";
export {
  discoverSkills,
  parseSkillMarkdown,
  installSkillSource,
  syncManagedSkillsToTargets,
  removeManagedSkill,
} from "./commands/skills.command.js";
export type {
  SkillDiscoveryScope,
  SkillRecord,
  SkillDiagnostic,
  SkillInstallResult,
} from "./commands/skills.command.js";
