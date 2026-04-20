import type { HandoffLaunchPort, LaunchStorePort } from "./domain/launch-types.js";
import { ClaudeHandoffLaunchAdapter } from "./adapters/claude-handoff-launch.adapter.js";
import { CodexHandoffLaunchAdapter } from "./adapters/codex-handoff-launch.adapter.js";
import { FsLaunchStoreAdapter } from "./adapters/launch-store.adapter.js";

export interface HandoffServices {
  readonly launchStore: LaunchStorePort;
  readonly handoffLaunchers: {
    readonly codex: HandoffLaunchPort;
    readonly claude: HandoffLaunchPort;
  };
}

export function buildHandoffServices(projectDir: string): HandoffServices {
  return {
    launchStore: new FsLaunchStoreAdapter(projectDir),
    handoffLaunchers: {
      codex: new CodexHandoffLaunchAdapter(),
      claude: new ClaudeHandoffLaunchAdapter(),
    },
  };
}
