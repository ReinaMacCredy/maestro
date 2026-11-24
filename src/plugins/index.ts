import type { BuiltInPlugin } from "../kernel/loader.ts";
import { briefPlugin } from "./brief.ts";
import { bundlePlugin } from "./bundle.ts";
import { coordinationPlugin } from "./coordination.ts";
import { decisionPlugin } from "./decision.ts";
import { installPlugin } from "./install.ts";
import { legacyPlugin } from "./legacy.ts";
import { lessonPlugin } from "./lesson.ts";
import { lifecyclePlugin } from "./lifecycle.ts";
import { mcpPlugin } from "./mcp.ts";
import { observabilityPlugin } from "./observability.ts";
import { pluginManagerPlugin } from "./plugin-manager.ts";
import { pluginTrustPlugin } from "./plugin-trust.ts";
import { policyAutoPrPlugin } from "./policy-auto-pr.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyCardBudgetPlugin } from "./policy-card-budget.ts";
import { policyLayerPlugin } from "./policy-layer.ts";
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
import { topicPlugin } from "./topic.ts";
import { memoryPlugin } from "./memory.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  pluginTrustPlugin,
  briefPlugin,
  workPlugin,
  decisionPlugin,
  lessonPlugin,
  coordinationPlugin,
  attentionPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
  policyCardBudgetPlugin,
  policyTddPlugin,
  policyQaPlugin,
  policyResearchPlugin,
  policyWitnessPlugin,
  policyLayerPlugin,
  policyLifecyclePlugin,
  policyAutoPrPlugin,
  recipePlugin,
  observabilityPlugin,
  termPlugin,
  topicPlugin,
  memoryPlugin,
  bundlePlugin,
  legacyPlugin,
  mcpPlugin,
  versionPlugin,
  installPlugin,
  slpV2Plugin,
  lifecyclePlugin,
];
