/**
 * Check every generated agent-facing spec embed before release/install flows.
 */
import { syncBundledSkills } from "./sync-bundled-skills";

await syncBundledSkills({ check: true });
