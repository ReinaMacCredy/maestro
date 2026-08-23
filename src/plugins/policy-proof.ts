import type { BuiltInPlugin } from "../kernel/loader.ts";

export const policyProofPlugin: BuiltInPlugin = {
  name: "policy-proof",
  inject: ["work"],
  apply(context) {
    context.effect(() =>
      context.cli.registerFlag("work done", "--claim", { value: true, multiple: true }),
    );
    context.effect(() =>
      context.cli.registerFlag("work done", "--proof", { value: true, multiple: true }),
    );
    context.effect(() =>
      context.events.on<{
        claims: string[];
        evidence: string;
        proofs: string[];
      }>("work.done", async (input, next) => {
        if (input.claims.length === 0) return next();
        const matched =
          input.proofs.length === input.claims.length &&
          input.proofs.every((proof) => proof.length > 0 && input.evidence.includes(proof));
        if (!matched) {
          return {
            blocked: true,
            origin: "policy-proof",
            reason: "each claim requires a matching proof present in evidence",
          };
        }
        return next();
      }),
    );
  },
};
