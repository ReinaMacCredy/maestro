import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { getServices, type Services } from "@/services.js";
import { amendContract } from "../usecases/amend-contract.usecase.js";
import { getCurrentContract } from "../usecases/get-current-contract.usecase.js";
import { getContractHistory } from "../usecases/get-contract-history.usecase.js";
import {
  contractSprint,
  formatContractSprintLines,
} from "../usecases/contract-sprint.usecase.js";
import { generateContractAmendmentId } from "../domain/contract/contract-state.js";
import type { Contract, ContractAmendment } from "../domain/contract/contract-types.js";

interface ContractL2Deps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "contractStore" | "evidenceStore"
  >;
  readonly amendContract: typeof amendContract;
  readonly contractSprint?: typeof contractSprint;
}

export function registerContractL2Command(
  program: Command,
  deps: ContractL2Deps = { getServices, amendContract },
): void {
  const contractCmd = program
    .command("contract")
    .description("Versioned contract inspection and amendment (L2)");

  registerShowSubcommand(contractCmd, program, deps);
  registerAmendSubcommand(contractCmd, program, deps);
  registerHistorySubcommand(contractCmd, program, deps);
  registerSprintSubcommand(contractCmd, program, deps);
}

function registerSprintSubcommand(
  parent: Command,
  root: Command,
  deps: ContractL2Deps,
): void {
  parent
    .command("sprint")
    .description(
      "Show sprint snapshot (criteria, amendment budget) and optionally record a proposal",
    )
    .requiredOption("--task <id>", "Task id")
    .option("--propose <text>", "Record a sprint-contract proposal as evidence")
    .option("--proposed-by <actor>", "Optional actor id for the proposal")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);
      const fn = deps.contractSprint ?? contractSprint;
      const result = await fn(
        {
          contractVersionStore: services.contractVersionStore,
          contractStore: services.contractStore,
          evidenceStore: services.evidenceStore,
        },
        {
          taskId: opts.task,
          ...(typeof opts.propose === "string" ? { propose: opts.propose } : {}),
          ...(typeof opts.proposedBy === "string" ? { proposedBy: opts.proposedBy } : {}),
        },
      );
      output(isJson, result, formatContractSprintLines);
    });
}

// ─── show ────────────────────────────────────────────────────────────────────

function registerShowSubcommand(parent: Command, root: Command, deps: ContractL2Deps): void {
  parent
    .command("show")
    .description("Show the current (or a specific) versioned contract for a task")
    .requiredOption("--task <id>", "Task id")
    .option("--at-version <n>", "Show a specific version (default: current)", parsePositiveInt)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const versionN: number | undefined = opts.atVersion;

      let contract: Contract | undefined;

      if (versionN === undefined) {
        contract = await getCurrentContract(
          services.contractVersionStore,
          services.contractStore,
          taskId,
        );
        if (contract === undefined) {
          throw new MaestroError(`No versioned contract found for task ${taskId}`, [
            "Propose a contract first with `maestro task contract new <taskId>`",
          ]);
        }
      } else {
        const history = await getContractHistory(
          services.contractVersionStore,
          services.contractStore,
          taskId,
        );
        contract = history[versionN - 1];
        if (contract === undefined) {
          throw new MaestroError(
            `Version ${versionN} does not exist for task ${taskId} (${history.length} version(s) available)`,
            [
              `Valid versions: 1–${history.length}`,
              `Run \`maestro contract history --task ${taskId}\` to list all versions`,
            ],
          );
        }
      }

      output(isJson, contract, formatContract);
    });
}

function formatContract(c: Contract): string[] {
  const lines: string[] = [
    `[contract] ${c.id}`,
    `  Task:            ${c.taskId}`,
    `  Status:          ${c.status}`,
    `  Created:         ${c.createdAt}`,
    `  Intent:          ${c.intent}`,
  ];

  if (c.riskClass !== undefined) lines.push(`  Risk class:      ${c.riskClass}`);

  // scope
  lines.push(`  Scope.filesExpected: ${c.scope.filesExpected.join(", ") || "(none)"}`);
  if (c.scope.filesForbidden.length > 0) {
    lines.push(`  Scope.filesForbidden: ${c.scope.filesForbidden.join(", ")}`);
  }

  // budgets
  if (c.amendmentBudget !== undefined) {
    const b = c.amendmentBudget;
    lines.push(
      `  AmendmentBudget: max=${b.maxAmendments} maxPerAmend=${b.maxPathsPerAmendment}` +
        (b.forbiddenAmendmentPaths.length > 0
          ? ` forbidden=[${b.forbiddenAmendmentPaths.join(", ")}]`
          : ""),
    );
  }
  if (c.costBudget !== undefined) {
    const b = c.costBudget;
    lines.push(
      `  CostBudget:      retries=${b.maxRetries} wallClock=${b.maxWallClockSeconds}s` +
        (b.maxTokens !== undefined ? ` tokens=${b.maxTokens}` : ""),
    );
  }

  // doneWhen (per-criterion checkboxes — agents need to see why a contract
  // closed fulfilled vs broken; aggregate status alone hides the receipt-hint
  // auto-mark trail and which criterion is unmarked when status is broken).
  if (c.doneWhen.length === 0) {
    lines.push("  DoneWhen:        (none)");
  } else {
    lines.push(`  DoneWhen (${c.doneWhen.length}):`);
    for (const criterion of c.doneWhen) {
      const box = criterion.met === true ? "[x]" : "[ ]";
      const evidence = criterion.metEvidence ? ` -- ${criterion.metEvidence}` : "";
      lines.push(`    ${box} (${criterion.kind}) ${criterion.text}${evidence}`);
    }
  }

  // amendments
  if (c.amendments.length === 0) {
    lines.push("  Amendments:      (none)");
  } else {
    lines.push(`  Amendments (${c.amendments.length}):`);
    for (const a of c.amendments) {
      lines.push(`    [${a.id}] ${a.at} by ${a.by} — ${a.reason}`);
    }
  }

  // verdict (only present after close — explains why broken vs fulfilled)
  if (c.verdict !== undefined) {
    const v = c.verdict;
    lines.push(`  Verdict:         ${v.fulfilled ? "fulfilled" : "broken"} (computed ${v.computedAt})`);
    if (v.outOfScopeFiles.length > 0) {
      lines.push(`    out-of-scope: ${v.outOfScopeFiles.join(", ")}`);
    }
    if (v.forbiddenTouched.length > 0) {
      lines.push(`    forbidden:    ${v.forbiddenTouched.join(", ")}`);
    }
    if (v.unmetCriteria.length > 0) {
      lines.push(`    unmet:        ${v.unmetCriteria.map((u) => u.id).join(", ")}`);
    }
  }

  return lines;
}

// ─── amend ───────────────────────────────────────────────────────────────────

function registerAmendSubcommand(parent: Command, root: Command, deps: ContractL2Deps): void {
  parent
    .command("amend")
    .description("Amend the versioned contract for a task")
    .requiredOption("--task <id>", "Task id")
    .option("--add-path <p>", "Add a path to the scope (repeat for multiple)", collect, [] as string[])
    .option("--remove-path <p>", "Remove a path from the scope (repeat for multiple)", collect, [] as string[])
    .requiredOption("--reason <str>", "Reason for the amendment")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const addPaths: string[] = opts.addPath ?? [];
      const removePaths: string[] = opts.removePath ?? [];
      const reason: string = opts.reason;

      if (addPaths.length === 0 && removePaths.length === 0) {
        throw new MaestroError("At least one of --add-path or --remove-path is required", [
          "Specify paths to add or remove from the contract scope",
        ]);
      }

      const before = await getCurrentContract(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );
      if (before === undefined) {
        throw new MaestroError(`No versioned contract found for task ${taskId}`, [
          "Propose a contract before amending it",
        ]);
      }

      const amendmentId = generateContractAmendmentId();

      const { result: newFilesExpected, skipped: skippedAddPaths } =
        applyPathChangesWithReport(before.scope.filesExpected, addPaths, removePaths);

      // Check if scope actually changed
      const scopeChanged =
        newFilesExpected.length !== before.scope.filesExpected.length ||
        newFilesExpected.some((p, i) => p !== before.scope.filesExpected[i]);

      if (!scopeChanged) {
        throw new MaestroError("No scope changes to apply", [
          "All paths are already covered by existing scope patterns or were removed",
        ]);
      }

      const amendment: ContractAmendment = {
        id: amendmentId,
        at: new Date().toISOString(),
        by: "maestro-cli",
        reason,
        before: {
          scope: before.scope,
        },
        after: {
          scope: {
            filesExpected: newFilesExpected,
            filesForbidden: before.scope.filesForbidden,
          },
        },
      };

      const { newVersion } = await deps.amendContract(
        services.contractVersionStore,
        services.contractStore,
        services.evidenceStore,
        {
          taskId,
          amendment,
          addedPaths: addPaths,
          removedPaths: removePaths,
        },
      );

      const result = { amendmentId, newVersion, skippedAddPaths };
      output(isJson, result, () => {
        const lines = [
          `[ok] Contract amended for task ${taskId}`,
          `  Amendment id:  ${amendmentId}`,
          `  New version:   ${newVersion}`,
        ];
        if (skippedAddPaths.length > 0) {
          lines.push(
            `  Skipped (already covered): ${skippedAddPaths.join(", ")}`,
          );
        }
        return lines;
      });
    });
}

function applyPathChangesWithReport(
  existing: readonly string[],
  add: readonly string[],
  remove: readonly string[],
): { result: string[]; skipped: string[] } {
  const removeSet = new Set(remove);
  const result = existing.filter((p) => !removeSet.has(p));
  const skipped: string[] = [];
  for (const p of add) {
    if (result.includes(p) || matchesAnyGlob(result, p)) {
      skipped.push(p);
      continue;
    }
    result.push(p);
  }
  return { result, skipped };
}

// ─── history ─────────────────────────────────────────────────────────────────

function registerHistorySubcommand(parent: Command, root: Command, deps: ContractL2Deps): void {
  parent
    .command("history")
    .description("List all versioned contract snapshots for a task in ascending order")
    .requiredOption("--task <id>", "Task id")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const history = await getContractHistory(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );

      output(isJson, history, formatHistory);
    });
}

function formatHistory(versions: readonly Contract[]): string[] {
  if (versions.length === 0) return ["No versioned contracts found."];
  return versions.map(
    (c, i) =>
      `v${i + 1}  ${c.createdAt}  ${c.status}  ${c.id}  amendments=${c.amendments.length}`,
  );
}

// ─── helpers ─────────────────────────────────────────────────────────────────

function collect(val: string, prev: string[]): string[] {
  return [...prev, val];
}

function parsePositiveInt(raw: string): number {
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n) || n < 1) {
    throw new MaestroError(`Invalid version: ${raw}`, ["Pass a positive integer (1 or greater)"]);
  }
  return n;
}
