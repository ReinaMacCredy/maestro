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
import { policyAutoPrPlugin } from "./policy-auto-pr.ts";
import { policyBreakdownPlugin } from "./policy-breakdown.ts";
import { policyCardBudgetPlugin } from "./policy-card-budget.ts";
import { policyProofPlugin } from "./policy-proof.ts";
import { recipePlugin } from "./recipe.ts";
import { attentionPlugin } from "./attention.ts";
import { versionPlugin } from "./version.ts";
import { workPlugin } from "./work.ts";
import { slpPlugin } from "./slp.ts";
import { termPlugin } from "./term.ts";
import { topicPlugin } from "./topic.ts";
import { memoryPlugin } from "./memory.ts";

export const builtInPlugins: readonly BuiltInPlugin[] = [
  pluginManagerPlugin,
  briefPlugin,
  workPlugin,
  decisionPlugin,
  lessonPlugin,
  coordinationPlugin,
  attentionPlugin,
  policyProofPlugin,
  policyBreakdownPlugin,
  policyCardBudgetPlugin,
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
  slpPlugin,
  lifecyclePlugin,
];
