import type { BuiltInPlugin } from "../kernel/loader.ts";
import { pluginManagerPlugin } from "./plugin-manager.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyProofPlugin } from "./policy-proof.ts";
import { workPlugin } from "./work.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  workPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
];
