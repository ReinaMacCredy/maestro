import type {
  HandoffLaunchPort,
  HandoffLaunchRequest,
  HandoffLaunchResult,
} from "@/features/handoff";
import { runLoggedCommand } from "@/shared/lib/shell.js";

export class HermesHandoffLaunchAdapter implements HandoffLaunchPort {
  readonly agent = "hermes" as const;

  async launch(request: HandoffLaunchRequest): Promise<HandoffLaunchResult> {
    const command = [
      "hermes",
      "chat",
      "--quiet",
      "--yolo",
      "--toolsets",
      "terminal,skills",
      "--source",
      "maestro",
      ...(request.modelProvided ? ["--model", request.model] : []),
      "-q",
      request.prompt,
    ];
    return runLoggedCommand(command, {
      cwd: request.targetDir,
      logPath: request.logPath,
      wait: request.wait,
      env: request.env,
    });
  }
}
