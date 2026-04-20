import type {
  HandoffLaunchPort,
  HandoffLaunchRequest,
  HandoffLaunchResult,
} from "@/features/handoff";
import { runLoggedCommand } from "@/shared/lib/shell.js";

export class CodexHandoffLaunchAdapter implements HandoffLaunchPort {
  readonly provider = "codex" as const;

  async launch(request: HandoffLaunchRequest): Promise<HandoffLaunchResult> {
    const command = [
      "codex",
      "exec",
      "--cd",
      request.targetDir,
      "--full-auto",
      "--model",
      request.model,
      request.prompt,
    ];
    return runLoggedCommand(command, {
      cwd: request.targetDir,
      logPath: request.logPath,
      wait: request.wait,
    });
  }
}
