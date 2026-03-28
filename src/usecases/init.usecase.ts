import type { ConfigPort } from "../ports/config.port.js";
import type { MaestroConfig } from "../domain/types.js";
import { ensureDir } from "../lib/fs.js";
import { homedir } from "node:os";
import { join } from "node:path";

export interface InitResult {
  readonly created: string[];
  readonly scope: "global" | "project";
}

const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
};

export async function initMaestro(
  config: ConfigPort,
  opts: { global: boolean; dir: string },
): Promise<InitResult> {
  const scope = opts.global ? "global" : "project";
  const created: string[] = [];

  if (opts.global) {
    const globalDir = join(homedir(), ".maestro");
    await ensureDir(globalDir);
    created.push(globalDir);

    if (!(await config.exists("global", opts.dir))) {
      await config.write("global", opts.dir, DEFAULT_CONFIG);
      created.push(join(globalDir, "config.yaml"));
    }
  } else {
    const maestroDir = join(opts.dir, ".maestro");
    const handoffsDir = join(maestroDir, "handoffs");
    await ensureDir(maestroDir);
    await ensureDir(handoffsDir);
    created.push(maestroDir, handoffsDir);

    if (!(await config.exists("project", opts.dir))) {
      await config.write("project", opts.dir, DEFAULT_CONFIG);
      created.push(join(maestroDir, "config.yaml"));
    }
  }

  return { created, scope };
}
