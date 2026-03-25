/**
 * maestro skill-list -- list available skills.
 */

import { defineCommand } from 'citty';
import { listSkills, type SkillEntry } from '../../../../skills/registry.ts';
import { output, renderTable } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

function formatSkillList(skills: Array<SkillEntry>): string {
  if (skills.length === 0) return 'No skills available.';
  const hasHints = skills.some(s => s.argumentHint);
  if (hasHints) {
    const rows = skills.map(s => [s.name, s.source, s.argumentHint ?? '', s.description]);
    return renderTable(['Name', 'Source', 'Args', 'Description'], rows);
  }
  const rows = skills.map(s => [s.name, s.source, s.description]);
  return renderTable(['Name', 'Source', 'Description'], rows);
}

export default defineCommand({
  meta: { name: 'skill-list', description: 'List available skills' },
  args: {},
  async run() {
    try {
      const skills = await listSkills();
      output(skills, formatSkillList);
    } catch (err) {
      handleCommandError('skill-list', err);
    }
  },
});
