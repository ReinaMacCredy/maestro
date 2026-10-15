import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { BriefService } from "./coordination.ts";

export const policyAutoPrPlugin: BuiltInPlugin = {
  name: "policy-auto-pr",
  defaultDisabled: true,
  inject: ["brief"],
  requires:
    "grants delivery, no gates: the deliver step (maestro recipe show opening-a-pr) commits, pushes the branch and opens a PR; merge, tag and release stay owner gates",
  apply(context) {
    const brief = context.brief as BriefService;
    context.effect(() =>
      brief.register(
        () =>
          "delivery grant: policy-auto-pr enabled; the deliver step (maestro recipe show opening-a-pr) commits, pushes the branch and opens a PR; merge, tag and release stay owner gates",
        { events: ["SessionStart"] },
      ),
    );
  },
};
