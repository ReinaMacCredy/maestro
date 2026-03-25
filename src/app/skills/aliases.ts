import type { BuiltinSkillName } from './registry.generated.ts';

/**
 * Maps old (pre-colon) skill names to new colon-prefixed names.
 * Used for backward compatibility with deprecation warnings.
 *
 * Extracted to a standalone module so consumers (e.g., config.ts) can import
 * this without pulling in the full ~380KB registry.generated.ts at startup.
 */
export const SKILL_ALIASES: Record<string, BuiltinSkillName> = {
  'writing-plans': 'maestro:design',
  'executing-plans': 'maestro:implement',
  'code-reviewer': 'maestro:review',
  'agents-md-mastery': 'maestro:agents-md',
  'brainstorming': 'maestro:brainstorming',
  'dispatching-parallel-agents': 'maestro:dispatching',
  'docker-mastery': 'maestro:docker',
  'parallel-exploration': 'maestro:parallel-exploration',
  'prompt-leverage': 'maestro:prompt-leverage',
  'systematic-debugging': 'maestro:debugging',
  'test-driven-development': 'maestro:tdd',
  'verification-before-completion': 'maestro:verification',
  'new-track': 'maestro:new-feature',
  'maestro:new-track': 'maestro:new-feature',
  // Overhaul redirects (Sprint 16)
  'maestro:setup': 'maestro:design',
  'maestro:status': 'maestro:design',
  'setup': 'maestro:design',
  'status': 'maestro:design',
};
