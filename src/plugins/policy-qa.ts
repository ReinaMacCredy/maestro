import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord, WorkService } from "./work.ts";

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

function hasTaggedPair(input: CompletionInput): boolean {
  return input.claims.some(
    (claim, index) => claim.startsWith("qa:") && (input.proofs[index]?.length ?? 0) > 0,
  );
}

export const policyQaPlugin: BuiltInPlugin = {
  name: "policy-qa",
  defaultDisabled: true,
  inject: ["work"],
  apply(context) {
    const work = context.work as WorkService;
    context.effect(() =>
      context.events.on<CompletionInput, CompletionResult>("work.done", async (input, next) => {
        if (work.children(input.work.id).length === 0 || hasTaggedPair(input)) return next();
        return {
          blocked: true,
          origin: "policy-qa",
          reason:
            `parent completion requires a qa: claim/proof pair; run: maestro work done ` +
            `${input.work.id} --claim "qa: <checked behavior>" --proof "<QA evidence>"`,
        };
      }),
    );
  },
};
