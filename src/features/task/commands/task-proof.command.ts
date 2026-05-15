import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { buildProofMap, type ProofMap } from "../domain/proof-map.js";
import { readCurrentContractWithBackfill } from "../usecases/read-current-contract-with-backfill.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

interface TaskProofDeps {
  readonly getServices: () => Pick<Services, "taskStore" | "evidenceStore" | "specStore" | "contractVersionStore" | "contractStore">;
}

export function registerTaskProofCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskProofDeps,
): void {
  taskCmd
    .command("proof")
    .description("Show the ProofMap: join Spec acceptance criteria with Evidence rows")
    .requiredOption("--task <id>", "Task id")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      // 1. Resolve task
      const task = await services.taskStore.get(taskId);
      if (task === undefined) {
        throw new MaestroError(`Task ${taskId} not found`, [
          "Check the task id with 'maestro task list'",
        ]);
      }

      // 2. Load spec (if the task references a mission)
      const spec = task.missionId !== undefined
        ? await services.specStore.read(task.missionId)
        : undefined;

      // 2b. Load contract (for doneWhen criteria if no spec)
      const contract = await readCurrentContractWithBackfill(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );

      // 3. Load evidence rows for the task
      const evidenceRows = await services.evidenceStore.list({ task_id: taskId });

      // 4. Build proof map
      const proofMap = buildProofMap({ taskId, spec, contract, evidenceRows });

      // 4b. Advisory: empty proof map with no contract or spec means there's
      //     nothing to prove against. First-time agents read empty output as
      //     "covered"; surface the cause so they know what to do.
      const hasSource = spec !== undefined || contract !== undefined;
      const advisory = (!hasSource && proofMap.entries.length === 0)
        ? {
            warning: "no-criteria-source" as const,
            message: `task ${taskId} has neither a mission Spec nor a locked Contract; nothing to prove against`,
            hint: "Lock a Contract: maestro task contract new "
              + taskId
              + " --from default && maestro task contract lock "
              + taskId,
          }
        : undefined;

      // 5. Render
      if (isJson) {
        const payload = advisory ? { ...proofMap, ...advisory } : proofMap;
        process.stdout.write(JSON.stringify(payload) + "\n");
      } else {
        if (advisory) {
          console.log(advisory.message);
          console.log(`hint: ${advisory.hint}`);
          return;
        }
        printTextProofMap(proofMap);
      }

      // 6. Exit 0 — read-only verb
    });
}

// ─── helpers ─────────────────────────────────────────────────────────────────

function printTextProofMap(proofMap: ProofMap): void {
  if (proofMap.entries.length === 0) {
    console.log(`Proof map for task ${proofMap.taskId}: no criteria found`);
    return;
  }

  const { uncoveredCount, entries } = proofMap;
  const total = entries.length;
  const coveredCount = total - uncoveredCount;
  const source = proofMap.missionId ? "spec" : "contract";
  console.log(
    `Proof map for task ${proofMap.taskId} (${source}, ${coveredCount}/${total} covered):`,
  );
  for (const entry of entries) {
    const marker = entry.covered ? "[covered]" : "[uncovered]";
    const evidenceSuffix = entry.evidence.length > 0
      ? ` — ${entry.evidence.length} evidence row${entry.evidence.length !== 1 ? "s" : ""}`
      : "";
    console.log(`  ${marker} ${entry.criterionId}: ${entry.criterionText}${evidenceSuffix}`);
  }
}
