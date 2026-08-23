import type { BuiltInPlugin } from "../kernel/loader.ts";
import { coordinationPlugin } from "./coordination.ts";
import { decisionPlugin } from "./decision.ts";
import { installPlugin } from "./install.ts";
import { observabilityPlugin } from "./observability.ts";
import { pluginManagerPlugin } from "./plugin-manager.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyProofPlugin } from "./policy-proof.ts";
import { recipePlugin } from "./recipe.ts";
import { workPlugin } from "./work.ts";
import { watchPlugin } from "./watch.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  workPlugin,
  decisionPlugin,
  coordinationPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
  recipePlugin,
  observabilityPlugin,
  watchPlugin,
  installPlugin,
];
