import { homedir } from "node:os";
import type { HandoffAgent, HandoffLaunchPort, HandoffStorePort } from "./domain/handoff-types.js";
import { ClaudeHandoffLaunchAdapter } from "./adapters/claude-handoff-launch.adapter.js";
import { CodexHandoffLaunchAdapter } from "./adapters/codex-handoff-launch.adapter.js";
import { HermesHandoffLaunchAdapter } from "./adapters/hermes-handoff-launch.adapter.js";
import { FsHandoffStoreAdapter } from "./adapters/handoff-store.adapter.js";

export interface HandoffServices {
  readonly handoffStore: HandoffStorePort;
  readonly handoffLaunchers: Record<HandoffAgent, HandoffLaunchPort>;
}

export function buildHandoffServices(): HandoffServices {
  return {
    handoffStore: new FsHandoffStoreAdapter(homedir()),
    handoffLaunchers: {
      codex: new CodexHandoffLaunchAdapter(),
      claude: new ClaudeHandoffLaunchAdapter(),
      hermes: new HermesHandoffLaunchAdapter(),
    },
  };
}
