import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);
const layers = ["source", "artifact", "installed", "live", "journey"] as const;

interface CompletionInput {
  claims: string[];
  evidence: string;
  proofs: string[];
  work: WorkRecord;
}

interface CompletionResult {
  blocked: boolean;
  evidence?: string;
  origin?: string;
  reason?: string;
}

function layerGap(text: string): string | null {
  const proven = layers.filter((layer) => new RegExp(`\\(${layer}\\)`, "i").test(text));
  if (proven.length === 0) {
    return `tag each proof with the layer it reached: ${layers.map((layer) => `(${layer})`).join(" ")}`;
  }
  const next = layers[layers.indexOf(proven.at(-1)!) + 1];
  if (!next || new RegExp(`\\b${next}:\\s*(NOT TESTED|n/a)`, "i").test(text)) return null;
  return `name the first unproven layer: append "${next}: NOT TESTED" (or "${next}: n/a")`;
}

export const policyLayerPlugin: BuiltInPlugin = {
  name: "policy-layer",
  defaultDisabled: true,
  inject: ["work"],
  requires:
    "gates work done on write-like work: proof must tag a reached layer (source|artifact|installed|live|journey) and name the next one NOT TESTED or n/a",
  apply(context) {
    context.effect(() =>
      context.events.on<CompletionInput, CompletionResult>("work.done", async (input, next) => {
        if (!writeLikeKinds.has(input.work.kind)) return next();
        const gap = layerGap([input.evidence, ...input.proofs].join("\n"));
        if (!gap) return next();
        return {
          blocked: true,
          origin: "policy-layer",
          reason:
            `${gap}; e.g. maestro work done ${input.work.id} --evidence ` +
            `"suite 135 pass @ <commit> (source); artifact: NOT TESTED"`,
        };
      }),
    );
  },
};
