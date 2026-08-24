import type { BuiltInPlugin } from "../kernel/loader.ts";

// Ships dark (D8): bundles may carry the whole method as prose; a stage-gate
// overlay is built only if usage proves the need. Enabling it today changes
// nothing.
export const policyLifecyclePlugin: BuiltInPlugin = {
  name: "policy-lifecycle",
  requires:
    "reserved: stage-gate overlay for bundle-driven work; defines no gates yet — enabling it changes nothing",
  apply() {},
};
