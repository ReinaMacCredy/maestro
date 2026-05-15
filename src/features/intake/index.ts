export type {
  IntakeFlag,
  IntakeLane,
  IntakeInput,
  IntakeResult,
  WorkType,
} from "./domain/types.js";
export { WORK_TYPES } from "./domain/types.js";
export {
  classifyWorkType,
  detectHarnessImpact,
  generateNextSteps,
} from "./domain/classify-work-type.js";
export { classifyIntake } from "./usecases/classify-intake.usecase.js";
export { registerIntakeCommand } from "./commands/intake.command.js";
export { buildIntakeServices } from "./services.js";
export type { IntakeServices } from "./services.js";
