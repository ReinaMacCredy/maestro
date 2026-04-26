/**
 * Check every generated agent-facing spec embed before release/install flows.
 */
import { syncBuiltInSkills } from "./sync-built-in-skills";
import { syncBundledSkills } from "./sync-bundled-skills";

await syncBuiltInSkills({ check: true });
await syncBundledSkills({ check: true });
