import type { MaestroConfig } from "./types.js";

export const MAESTRO_DIR = ".maestro";

export const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
};

export const CASS_INSTALL_HINT = "Install: brew install dicklesworthstone/tap/cass";
