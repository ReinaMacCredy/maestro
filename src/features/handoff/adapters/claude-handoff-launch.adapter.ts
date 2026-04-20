import type {
  HandoffLaunchPort,
  HandoffLaunchRequest,
  HandoffLaunchResult,
} from "@/features/handoff";
import { runLoggedCommand } from "@/shared/lib/shell.js";

export class ClaudeHandoffLaunchAdapter implements HandoffLaunchPort {
  readonly provider = "claude" as const;

  async launch(request: HandoffLaunchRequest): Promise<HandoffLaunchResult> {
    const command = [
      "claude",
      "--print",
      "--permission-mode",
      "bypassPermissions",
      "--model",
      request.model,
      "--name",
      request.name,
      request.prompt,
    ];
    return runLoggedCommand(command, {
      cwd: request.targetDir,
      logPath: request.logPath,
      wait: request.wait,
    });
  }
}
