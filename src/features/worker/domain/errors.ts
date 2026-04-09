// Worker feature error factories.
// Moved from src/domain/errors.ts in Phase 6.

import { MaestroError } from "@/shared/errors.js";

export function workerSkillNotFound(workerType: string): MaestroError {
  return new MaestroError(`Worker skill '${workerType}' not found`, [
    `Create skill at .maestro/skills/${workerType}/SKILL.md`,
    `Or add a built-in skill at skills/built-in/${workerType}/SKILL.md`,
    "Skills define the worker's behavior, report format, and handoff protocol",
  ]);
}
