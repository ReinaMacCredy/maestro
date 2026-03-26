/**
 * maestro skill <name> -- load and print a skill template.
 */

import { defineCommand } from 'citty';
import { loadSkill, loadSkillReference } from '../../../../app/skills/registry.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';

function formatSkillContent(result: { content: string }): string {
  return result.content;
}

export default defineCommand({
  meta: { name: 'skill', description: 'Load and print a skill template\n\nExamples:\n  maestro skill maestro:design\n  maestro skill maestro:parallel-exploration --json' },
  args: {
    name: {
      type: 'positional',
      description: 'Skill name to load',
      required: true,
    },
    ref: {
      type: 'string',
      description: 'Load a specific reference file from the skill (e.g. steps/step-01.md)',
    },
  },
  async run({ args }) {
    try {
      const result = args.ref
        ? await loadSkillReference(args.name, args.ref)
        : await loadSkill(args.name);

      if ('error' in result) {
        throw new MaestroError(result.error);
      }

      output(result, formatSkillContent);
    } catch (err) {
      handleCommandError('skill', err);
    }
  },
});
