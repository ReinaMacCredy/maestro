/**
 * Validation command handler
 * Implements CLI commands: validate show|update
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import {
  showAssertions,
  updateAssertion,
  type ShowAssertionsResult,
  type UpdateAssertionResult,
} from "../usecases/validation-lifecycle.usecase.js";
import { MaestroError } from "../domain/errors.js";
import type { Assertion } from "../domain/mission-types.js";

export function registerValidateCommand(program: Command): void {
  const validationCmd = program
    .command("validate")
    .description("Validation lifecycle management")
    .option("--json", "Output as JSON");

  validationCmd
    .command("show")
    .description("Show assertions for a mission")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--milestone <id>", "Filter by milestone ID")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.mission) {
        throw new MaestroError("--mission is required", [
          "Usage: maestro validate show --mission <id>",
          "Optional filter: --milestone <milestoneId>",
        ]);
      }

      const result = await showAssertions(
        services.missionStore,
        services.assertionStore,
        opts.mission,
        opts.milestone,
      );

      output(isJson, result, formatAssertionList);
    });

  validationCmd
    .command("update <assertionId>")
    .description("Update assertion result with evidence or waived reason")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--result <result>", "New result (pending, passed, failed, blocked, waived)")
    .option("--evidence <text>", "Evidence or notes for this result")
    .option("--reason <reason>", "Required when result is 'waived'")
    .option("--json", "Output as JSON")
    .action(async (assertionId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.mission) {
        throw new MaestroError("--mission is required", [
          "Usage: maestro validate update <assertionId> --mission <id> --result <result>",
          "Optional: --evidence <text> --reason <reason>",
        ]);
      }

      if (!opts.result) {
        throw new MaestroError("--result is required", [
          "Usage: maestro validate update <assertionId> --mission <id> --result <result>",
          "Valid results: pending, passed, failed, blocked, waived",
          "Use --reason when result is 'waived'",
        ]);
      }

      const result = await updateAssertion(
        services.missionStore,
        services.assertionStore,
        opts.mission,
        assertionId,
        {
          result: opts.result,
          evidence: opts.evidence,
          waivedReason: opts.reason,
        },
      );

      output(isJson, result, formatAssertionUpdate);
    });
}

/** Format assertion list for text output */
function formatAssertionList(result: ShowAssertionsResult): string[] {
  if (result.assertions.length === 0) {
    return result.milestoneId
      ? [`No assertions found for milestone ${result.milestoneId}`]
      : ["No assertions found"];
  }

  const lines: string[] = [];

  if (result.milestoneId) {
    lines.push(`${result.filtered} assertion(s) for milestone ${result.milestoneId} (total: ${result.total})`);
  } else {
    lines.push(`${result.filtered} assertion(s) (total: ${result.total})`);
  }

  lines.push("");

  for (const a of result.assertions) {
    const status = a.result.padEnd(10);
    const desc = a.description.slice(0, 40).padEnd(40);
    lines.push(`${a.id}  ${status}  ${desc}  [${a.featureId}]`);

    if (a.evidence) {
      lines.push(`      Evidence: ${a.evidence.slice(0, 60)}${a.evidence.length > 60 ? "..." : ""}`);
    }

    if (a.waivedReason) {
      lines.push(`      Waived: ${a.waivedReason.slice(0, 60)}${a.waivedReason.length > 60 ? "..." : ""}`);
    }
  }

  return lines;
}

/** Format assertion update result for text output */
function formatAssertionUpdate(result: UpdateAssertionResult): string[] {
  const lines: string[] = [
    `[ok] Assertion updated: ${result.assertion.id}`,
    `  Result: ${result.assertion.result}`,
    `  Description: ${result.assertion.description}`,
    `  Feature: ${result.assertion.featureId}`,
    `  Milestone: ${result.assertion.milestoneId}`,
  ];

  if (result.assertion.evidence) {
    lines.push(`  Evidence: ${result.assertion.evidence}`);
  }

  if (result.assertion.waivedReason) {
    lines.push(`  Waived Reason: ${result.assertion.waivedReason}`);
  }

  return lines;
}
