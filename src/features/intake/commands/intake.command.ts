import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { classifyIntake } from "../usecases/classify-intake.usecase.js";
import type { IntakeFlag, IntakeResult } from "../domain/types.js";

const VALID_FLAGS: ReadonlySet<IntakeFlag> = new Set([
  "auth",
  "authz",
  "data-model",
  "audit-security",
  "external-systems",
  "public-contracts",
  "cross-platform",
  "existing-behavior",
  "weak-proof",
  "multi-domain",
]);

interface IntakeCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "getEffectiveRiskPolicy" | "getEffectiveSensitivePathsGlobs"
  >;
}

export function registerIntakeCommand(
  program: Command,
  deps: IntakeCommandDeps = { getServices },
): void {
  program
    .command("intake")
    .description("Plan-time risk classifier; returns a lane and recommended next step before coding")
    .option(
      "--paths <list>",
      "Comma-separated list of intended file paths",
      (val: string) => val.split(",").map((s) => s.trim()).filter((s) => s.length > 0),
    )
    .option(
      "--flag <flag>",
      "Declare a risk flag (repeatable). One of: " + Array.from(VALID_FLAGS).join(", "),
      (val: string, acc: IntakeFlag[]) => {
        if (!VALID_FLAGS.has(val as IntakeFlag)) {
          throw new MaestroError(`Unknown intake flag: ${val}`, [
            `Valid flags: ${Array.from(VALID_FLAGS).join(", ")}`,
          ]);
        }
        if (acc.includes(val as IntakeFlag)) {
          throw new MaestroError(`Duplicate intake flag: ${val}`, [
            "Each --flag value can be provided at most once.",
          ]);
        }
        acc.push(val as IntakeFlag);
        return acc;
      },
      [] as IntakeFlag[],
    )
    .option("--json", "Output as JSON")
    .addHelpText(
      "after",
      `
Lanes:
  tiny        0-1 flags, no hard gate. Patch directly, run validation, close.
  normal      2-3 flags, no hard gate. Create task via \`maestro task plan\`.
  high-risk   any hard gate, OR 4+ flags. Require Spec + threat-model.

Hard gates (any one promotes to high-risk):
  auth, authz, data-model, audit-security, external-systems

Examples:
  maestro intake --paths src/foo.ts,src/bar.ts
  maestro intake --paths src/auth/session.ts --flag auth
  maestro intake --paths .maestro/policies/risk.yaml --json

Exit code is always 0; agents react to the lane in the output.
`,
    )
    .action(async (opts: {
      paths?: string[];
      flag: IntakeFlag[];
      json?: boolean;
    }) => {
      const isJson = resolveJsonFlag(opts, program);
      const paths: readonly string[] = opts.paths ?? [];

      if (paths.length === 0) {
        throw new MaestroError("--paths is required (pass at least one path)", [
          "maestro intake --paths src/foo.ts,src/bar.ts",
        ]);
      }

      const services = deps.getServices();
      const [riskPolicy, sensitivePaths] = await Promise.all([
        services.getEffectiveRiskPolicy(),
        services.getEffectiveSensitivePathsGlobs(),
      ]);

      const result = classifyIntake(
        {
          intendedPaths: paths,
          declaredFlags: opts.flag,
        },
        riskPolicy,
        sensitivePaths,
      );

      if (isJson) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        printResult(result);
      }
    });
}

function printResult(r: IntakeResult): void {
  const laneMarker = r.lane === "tiny" ? "[tiny]" : r.lane === "normal" ? "[normal]" : "[!! high-risk]";
  console.log(`${laneMarker} lane=${r.lane} derivedRiskClass=${r.derivedRiskClass}`);
  if (r.derivedRiskSignal !== undefined) {
    console.log(`         signal=${r.derivedRiskSignal}`);
  }
  if (r.autoDetectedFlags.length > 0) {
    console.log(`         auto-detected flags: ${r.autoDetectedFlags.join(", ")}`);
  }
  if (r.declaredFlags.length > 0) {
    console.log(`         declared flags:      ${r.declaredFlags.join(", ")}`);
  }
  if (r.hardGatesTriggered.length > 0) {
    console.log(`         hard gates:          ${r.hardGatesTriggered.join(", ")}`);
  }
  if (r.threatModelRequired) {
    console.log(`         threat-model:        required (Evidence row of kind=threat-model)`);
  }
  console.log("");
  console.log(`Next step: ${r.recommendedNextStep}`);
}
