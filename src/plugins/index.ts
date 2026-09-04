import type { BuiltInPlugin } from "../kernel/loader.ts";
import { briefPlugin } from "./brief.ts";
import { bundlePlugin } from "./bundle.ts";
import { coordinationPlugin } from "./coordination.ts";
import { decisionPlugin } from "./decision.ts";
import { dispatchPlugin } from "./dispatch.ts";
import { importRustPlugin } from "./import-rust.ts";
import { installPlugin } from "./install.ts";
import { lessonPlugin } from "./lesson.ts";
import { lifecyclePlugin } from "./lifecycle.ts";
import { mcpPlugin } from "./mcp.ts";
import { observabilityPlugin } from "./observability.ts";
import { pluginManagerPlugin } from "./plugin-manager.ts";
import { pluginTrustPlugin } from "./plugin-trust.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyCardBudgetPlugin } from "./policy-card-budget.ts";
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
import { slpV2Plugin } from "./slp-v2.ts";
import { termPlugin } from "./term.ts";
import { memoryPlugin } from "./memory.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  pluginTrustPlugin,
  briefPlugin,
  workPlugin,
  dispatchPlugin,
  decisionPlugin,
  lessonPlugin,
  coordinationPlugin,
  attentionPlugin,
  policyDispatchPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
  policyCardBudgetPlugin,
  policyTddPlugin,
  policyQaPlugin,
  policyResearchPlugin,
  policyWitnessPlugin,
  policyLifecyclePlugin,
  recipePlugin,
  observabilityPlugin,
  termPlugin,
  memoryPlugin,
  bundlePlugin,
  importRustPlugin,
  mcpPlugin,
  versionPlugin,
  installPlugin,
  slpV2Plugin,
  lifecyclePlugin,
];
