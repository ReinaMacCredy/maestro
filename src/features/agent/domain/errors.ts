import { MaestroError } from "@/shared/errors.js";

export function workerSkillNotFound(agentType: string): MaestroError {
  return new MaestroError(`Worker skill '${agentType}' not found`, [
    `Create skill at .maestro/skills/${agentType}/SKILL.md`,
    `Or add a built-in skill at skills/built-in/${agentType}/SKILL.md`,
    "Skills define the worker's behavior, report format, and handoff protocol",
  ]);
}
