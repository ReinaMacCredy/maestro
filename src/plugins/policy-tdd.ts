import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord } from "./work.ts";

const writeLikeKinds = new Set(["feature", "task", "bug", "chore", "implement"]);

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
    (claim, index) => claim.startsWith("test:") && (input.proofs[index]?.length ?? 0) > 0,
  );
}

export const policyTddPlugin: BuiltInPlugin = {
  name: "policy-tdd",
  defaultDisabled: true,
  inject: ["work"],
  apply(context) {
    context.effect(() =>
      context.events.on<CompletionInput, CompletionResult>("work.done", async (input, next) => {
        if (!writeLikeKinds.has(input.work.kind) || hasTaggedPair(input)) return next();
        return {
          blocked: true,
          origin: "policy-tdd",
          reason:
            `write-like completion requires a test: claim/proof pair; run: maestro work done ` +
            `${input.work.id} --claim "test: <test claim>" --proof "<test output>"`,
        };
      }),
    );
  },
};
