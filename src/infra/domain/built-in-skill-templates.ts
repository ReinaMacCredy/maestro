// Generated from skills/built-in so compiled releases can sync shipped skills.
// Edit the .md files under skills/built-in/ and run `bun scripts/sync-built-in-skills.ts`.
export interface BuiltInSkillFile {
  readonly path: string;
  readonly content: string;
}

export interface BuiltInSkillTemplate {
  readonly name: string;
  readonly files: readonly BuiltInSkillFile[];
}

export const BUILT_IN_SKILL_TEMPLATES: readonly BuiltInSkillTemplate[] =
[];
