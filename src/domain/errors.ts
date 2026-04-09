// Transitional shim kept alive until Phase 6 moves workerSkillNotFound
// into the worker feature. New code should import MaestroError from
// "@/shared/errors.js" and mission factories from "@/features/mission".

export { MaestroError } from "@/shared/errors.js";
import { MaestroError } from "@/shared/errors.js";

export function workerSkillNotFound(workerType: string): MaestroError {
  return new MaestroError(`Worker skill '${workerType}' not found`, [
    `Create skill at .maestro/skills/${workerType}/SKILL.md`,
    `Or add a built-in skill at skills/built-in/${workerType}/SKILL.md`,
    "Skills define the worker's behavior, report format, and handoff protocol",
  ]);
}
