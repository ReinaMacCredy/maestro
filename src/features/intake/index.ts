export type {
  IntakeFlag,
  IntakeLane,
  IntakeInput,
  IntakeResult,
} from "./domain/types.js";
export { classifyIntake } from "./usecases/classify-intake.usecase.js";
export { registerIntakeCommand } from "./commands/intake.command.js";
export { buildIntakeServices } from "./services.js";
export type { IntakeServices } from "./services.js";
