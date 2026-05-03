import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { amendContract } from "../usecases/amend-contract.usecase.js";
import { getCurrentContract } from "../usecases/get-current-contract.usecase.js";
import { getContractHistory } from "../usecases/get-contract-history.usecase.js";
import type { Contract, ContractAmendment } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";

interface ContractL2Deps {
  readonly getServices: () => Pick<Services, "contractVersionStore" | "evidenceStore">;
  readonly amendContract: typeof amendContract;
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
}

// ─── show ────────────────────────────────────────────────────────────────────

function registerShowSubcommand(parent: Command, root: Command, deps: ContractL2Deps): void {
  parent
    .command("show")
    .description("Show the current (or a specific) versioned contract for a task")
    .requiredOption("--task <id>", "Task id")
    .option("--version <n>", "Show a specific version (default: current)", parsePositiveInt)
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const versionN: number | undefined = opts.version;

      let contract: Contract | undefined;

      if (versionN === undefined) {
        contract = await getCurrentContract(services.contractVersionStore, taskId);
        if (contract === undefined) {
          throw new MaestroError(`No versioned contract found for task ${taskId}`, [
            "Propose a contract first with `maestro task contract new <taskId>`",
          ]);
        }
      } else {
        const history = await getContractHistory(services.contractVersionStore, taskId);
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

  // amendments
  if (c.amendments.length === 0) {
    lines.push("  Amendments:      (none)");
  } else {
    lines.push(`  Amendments (${c.amendments.length}):`);
    for (const a of c.amendments) {
      lines.push(`    [${a.id}] ${a.at} by ${a.by} — ${a.reason}`);
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
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const addPaths: string[] = opts.addPath ?? [];
      const removePaths: string[] = opts.removePath ?? [];
      const reason: string = opts.reason;

      // Fetch current version before amend to compute new version number
      const before = await getCurrentContract(services.contractVersionStore, taskId);
      if (before === undefined) {
        throw new MaestroError(`No versioned contract found for task ${taskId}`, [
          "Propose a contract before amending it",
        ]);
      }

      const amendmentId = generateAmendmentId();
      const now = new Date().toISOString();

      const amendment: ContractAmendment = {
        id: amendmentId,
        at: now,
        by: "maestro-cli",
        reason,
        before: {
          scope: before.scope,
        },
        after: {
          scope: {
            filesExpected: applyPathChanges(before.scope.filesExpected, addPaths, removePaths),
            filesForbidden: before.scope.filesForbidden,
          },
        },
      };

      await deps.amendContract(services.contractVersionStore, services.evidenceStore, {
        taskId,
        amendment,
        addedPaths: addPaths,
        removedPaths: removePaths,
      });

      // Determine new version number
      const history = await getContractHistory(services.contractVersionStore, taskId);
      const newVersion = history.length;

      const result = { amendmentId, newVersion };
      output(isJson, result, () => [
        `[ok] Contract amended for task ${taskId}`,
        `  Amendment id:  ${amendmentId}`,
        `  New version:   ${newVersion}`,
      ]);
    });
}

function applyPathChanges(
  existing: readonly string[],
  add: readonly string[],
  remove: readonly string[],
): string[] {
  const removeSet = new Set(remove);
  const result = existing.filter((p) => !removeSet.has(p));
  for (const p of add) {
    if (!result.includes(p)) result.push(p);
  }
  return result;
}

function generateAmendmentId(): string {
  // Must match AMENDMENT_ID_PATTERN: /^a-[0-9a-f]{6}$/
  const bytes = crypto.getRandomValues(new Uint8Array(3));
  const hex = Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
  return `a-${hex}`;
}

// ─── history ─────────────────────────────────────────────────────────────────

function registerHistorySubcommand(parent: Command, root: Command, deps: ContractL2Deps): void {
  parent
    .command("history")
    .description("List all versioned contract snapshots for a task in ascending order")
    .requiredOption("--task <id>", "Task id")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root);

      const taskId: string = opts.task;
      const history = await getContractHistory(services.contractVersionStore, taskId);

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
