import type { BuiltInPlugin } from "../kernel/loader.ts";
import { bundlePlugin } from "./bundle.ts";
import { coordinationPlugin } from "./coordination.ts";
import { decisionPlugin } from "./decision.ts";
import { dispatchPlugin } from "./dispatch.ts";
import { importRustPlugin } from "./import-rust.ts";
import { installPlugin } from "./install.ts";
import { lifecyclePlugin } from "./lifecycle.ts";
import { mcpPlugin } from "./mcp.ts";
import { observabilityPlugin } from "./observability.ts";
import { pluginManagerPlugin } from "./plugin-manager.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyLifecyclePlugin } from "./policy-lifecycle.ts";
import { policyProofPlugin } from "./policy-proof.ts";
import { policyQaPlugin } from "./policy-qa.ts";
import { policyResearchPlugin } from "./policy-research.ts";
import { policyTddPlugin } from "./policy-tdd.ts";
import { policyWitnessPlugin } from "./policy-witness.ts";
import { recipePlugin } from "./recipe.ts";
import { supervisorPlugin } from "./supervisor.ts";
import { versionPlugin } from "./version.ts";
import { workPlugin } from "./work.ts";
import { watchPlugin } from "./watch.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  workPlugin,
  dispatchPlugin,
  decisionPlugin,
  coordinationPlugin,
  supervisorPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
  policyTddPlugin,
  policyQaPlugin,
  policyResearchPlugin,
  policyWitnessPlugin,
  policyLifecyclePlugin,
  recipePlugin,
  observabilityPlugin,
  bundlePlugin,
  importRustPlugin,
  mcpPlugin,
  watchPlugin,
  versionPlugin,
  installPlugin,
  lifecyclePlugin,
];
