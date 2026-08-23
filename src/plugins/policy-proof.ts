import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord } from "./work.ts";

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

function recordPairs(input: CompletionInput): string {
  const pairs = input.claims
    .map((claim, index) => `claim: ${claim}\nproof: ${input.proofs[index] ?? ""}`)
    .join("\n");
  return input.evidence ? `${input.evidence}\n${pairs}` : pairs;
}

export const policyProofPlugin: BuiltInPlugin = {
  name: "policy-proof",
  inject: ["work"],
  apply(context) {
    context.effect(() =>
      context.cli.registerFlag("work done", "--claim", {
        description: "Record a completion claim.",
        value: true,
        multiple: true,
      }),
    );
    context.effect(() =>
      context.cli.registerFlag("work done", "--proof", {
        description: "Record proof paired with a claim.",
        value: true,
        multiple: true,
      }),
    );
    context.effect(() =>
      context.events.on<CompletionInput, CompletionResult>("work.done", async (input, next) => {
        if (input.claims.length === 0 && input.proofs.length === 0) {
          if (input.evidence.length > 0) return next();
          return {
            blocked: true,
            origin: "policy-proof",
            reason:
              `completion evidence is required; run: maestro work done ${input.work.id} ` +
              `--evidence "<evidence>", or maestro work done ${input.work.id} ` +
              `--claim "<claim>" --proof "<proof>"`,
          };
        }
        const paired =
          input.proofs.length === input.claims.length &&
          input.proofs.every((proof) => proof.length > 0);
        if (!paired) {
          return {
            blocked: true,
            origin: "policy-proof",
            reason:
              `each claim requires a non-empty paired proof; run: maestro work done ` +
              `${input.work.id} --claim "<claim>" --proof "<proof>"`,
          };
        }
        return next({ ...input, evidence: recordPairs(input) });
      }),
    );
  },
};
