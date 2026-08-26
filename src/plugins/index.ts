import type { BuiltInPlugin } from "../kernel/loader.ts";
import { briefPlugin } from "./brief.ts";
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
import { policyDispatchPlugin } from "./policy-dispatch.ts";
import { policyLifecyclePlugin } from "./policy-lifecycle.ts";
import { policyProofPlugin } from "./policy-proof.ts";
import { policyQaPlugin } from "./policy-qa.ts";
import { policyResearchPlugin } from "./policy-research.ts";
import { policyTddPlugin } from "./policy-tdd.ts";
import { policyWitnessPlugin } from "./policy-witness.ts";
import { recipePlugin } from "./recipe.ts";
import { attentionPlugin } from "./attention.ts";
import { versionPlugin } from "./version.ts";
import { workPlugin } from "./work.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  briefPlugin,
  workPlugin,
  dispatchPlugin,
  decisionPlugin,
  coordinationPlugin,
  attentionPlugin,
  policyDispatchPlugin,
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
  versionPlugin,
  installPlugin,
  lifecyclePlugin,
];
