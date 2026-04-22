import { Command, Option } from "commander";
import { getServices } from "@/services.js";
import { MaestroError } from "@/shared/errors.js";
import { readTextOrStdin } from "@/shared/lib/fs.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { createTask } from "../usecases/create-task.usecase.js";
import { showTask } from "../usecases/show-task.usecase.js";
import { listTasks } from "../usecases/list-tasks.usecase.js";
import { updateTask } from "../usecases/update-task.usecase.js";
import { claimTask } from "../usecases/claim-task.usecase.js";
import { unclaimTask } from "../usecases/unclaim-task.usecase.js";
import { blockTasks, unblockTasks } from "../usecases/manage-task-blockers.usecase.js";
import { releaseOwnedTasks } from "../usecases/release-owned-tasks.usecase.js";
import { readyTaskPage, readyTasks } from "../usecases/ready-tasks.usecase.js";
import { captureTaskCandidate } from "../usecases/capture-task-candidate.usecase.js";
import { planTasks } from "../usecases/plan-tasks.usecase.js";
import { findSimilarTasks } from "../usecases/find-similar-tasks.usecase.js";
import { heartbeatTask } from "../usecases/heartbeat-task.usecase.js";
import { closeContractForTask } from "../usecases/contract/close-contract.usecase.js";
import { computeContractVerdictForTask } from "../usecases/contract/compute-verdict.usecase.js";
import { loadContractForReopen } from "../usecases/contract/reopen-contract.usecase.js";
import { transferContractOwnership } from "../usecases/contract/transfer-ownership.usecase.js";
import { deleteTaskFlow } from "../usecases/delete-task-flow.usecase.js";
import {
  pruneLocalTaskState,
  type PruneKinds,
} from "../usecases/prune-local-task-state.usecase.js";
import { STUCK_THRESHOLD_MS, isStuckTask } from "../domain/now-md-format.js";
import { parseDuration } from "./duration.js";
import {
  buildTaskContinuationSummary,
  buildTaskOwnerId,
  buildTaskShowView,
  deriveAgentFromAssignee,
  loadTaskContinuationSummary,
  parseTaskOwnerId,
  syncTaskContinuation,
} from "../usecases/task-continuation.usecase.js";
import { reopenTaskFlow } from "../usecases/reopen-task-flow.usecase.js";
import type {
  ListTasksFilters,
  ReadyTasksFilters,
  Task,
  TaskMutationInput,
  TaskReceipt,
  UpdateTaskInput,
} from "../domain/task-types.js";
import { TASK_STATUSES, TASK_TYPES, buildTaskReceipt, indexTasksById } from "../domain/task-types.js";
import {
  buildCreateInput,
  hasAnyPatchField,
  parseCreateStatus,
  parseLimit,
  parseList,
  parsePlanInput,
  parsePriority,
  parseStatus,
  parseType,
} from "./task-command-parsers.js";
import {
  taskCompletedViaUpdateStatus,
  taskDependencyCommandsRenamed,
  taskUpdateClaimViaDedicatedCommand,
  taskUpdateOwnershipViaClaim,
} from "../domain/task-errors.js";
import { assertTaskMutationOwnership, assertTaskUpdateAllowed } from "../domain/task-state.js";
import {
  buildCompactReadyTaskPayload,
  formatPruneReport,
  formatTaskBriefingList,
  formatTaskDetail,
  formatTaskShowView,
  formatTaskList,
  formatTaskSummary,
} from "./task-command-formatters.js";
import { registerContractCommand } from "./contract.command.js";
import { syncTaskMetadata } from "../usecases/sync-task-metadata.usecase.js";
import { resolveTaskSilentMode } from "./command-silence.js";

interface ContinuationEditInput {
  readonly currentState?: string;
  readonly nextAction?: string;
  readonly addDecisions: readonly string[];
  readonly removeDecisions: readonly string[];
}

export function registerTaskCommand(program: Command): void {
  const taskCmd = program
    .command("task")
    .description("Task lifecycle management (Claude-style blocker graph)")
    .option("--json", "Output as JSON");

  registerCreateCommand(taskCmd, program);
  registerPlanCommand(taskCmd, program);
  registerQuickCommand(taskCmd, program);
  registerShowCommand(taskCmd, program);
  registerListCommand(taskCmd, program);
  registerUpdateCommand(taskCmd, program);
  registerClaimCommand(taskCmd, program);
  registerContractCommand(taskCmd, program);
  registerUnclaimCommand(taskCmd, program);
  registerReleaseOwnedCommand(taskCmd, program);
  registerBlockCommand(taskCmd, program);
  registerUnblockCommand(taskCmd, program);
  registerReopenCommand(taskCmd, program);
  registerDeleteCommand(taskCmd, program);
  registerPruneCommand(taskCmd, program);
  registerLegacyDepsCommand(taskCmd);
  registerCloseCommand(taskCmd);
  registerReadyCommand(taskCmd, program);
  registerSimilarCommand(taskCmd, program);
  registerMineCommand(taskCmd, program);
  registerStuckCommand(taskCmd, program);
  registerHeartbeatCommand(taskCmd, program);
}

function registerCreateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("create <title>")
    .description("Create a new task")
    .option("--description <text>", "Task description")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", "Priority 0-4 (0=critical, 4=backlog)")
    .option("--parent <id>", "Parent task id for hierarchy grouping")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--blocked-by <ids>", "Comma-separated blocker task ids")
    .option(
      "--status <status>",
      "Initial status: pending (default) or in_progress (auto-claims the task); completed is rejected",
    )
    .option("--session <id>", "Use an explicit session id instead of auto-detection (only with --status in_progress)")
    .addOption(new Option("--assignee <name>").hideHelp())
    .option("--silent", "Print only the id (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (opts.assignee !== undefined) {
        throw taskUpdateOwnershipViaClaim();
      }

      const initialStatus = parseCreateStatus(opts.status);

      if (opts.session !== undefined && initialStatus !== "in_progress") {
        throw new MaestroError("--session only applies with --status in_progress", [
          "Drop --session for a plain create (new tasks have no owner)",
          "Or pair --session with --status in_progress to auto-claim at create time",
        ]);
      }

      let sessionId: string | undefined;
      if (initialStatus === "in_progress") {
        sessionId = await resolveOwnershipSessionId(opts.session);
        await maybeReleaseStaleOwnedTasks([sessionId]);
      }

      const input = buildCreateInput(title, {
        description: opts.description,
        type: opts.type,
        priority: opts.priority,
        parent: opts.parent,
        labels: opts.labels,
        blockedBy: opts.blockedBy,
      });
      let task = await createTask(services.taskStore, input);

      if (initialStatus === "in_progress") {
        const { task: started, autoClaimed } = await updateTask(
          services.taskStore,
          task.id,
          { status: "in_progress" },
          { sessionId },
        );
        task = started;
        warnAutoClaimed(started, autoClaimed);
      }

      await refreshNowMd();

      if (opts.silent) {
        console.log(task.id);
        return;
      }

      output(isJson, task, formatTaskSummary);
    });
}

function registerPlanCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("plan")
    .description("Create a batch of tasks atomically from a JSON plan")
    .requiredOption("--file <path>", "Plan file path ('-' to read JSON from stdin)")
    .option("--start <name>", "After ingest, auto-claim and move the named batch-local task to in_progress")
    .option("--session <id>", "Use an explicit session id for --start (only with --start)")
    .option("--dry-run", "Validate + resolve references without writing any tasks")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const raw = await readPlanSource(opts.file);
      const batchInput = parsePlanInput(raw);

      if (opts.session !== undefined && opts.start === undefined) {
        throw new MaestroError("--session only applies with --start", [
          "Pair --session with --start <name> to auto-claim the named task",
          "Drop --session for a plan without start-of-work",
        ]);
      }

      let sessionId: string | undefined;
      if (opts.start !== undefined) {
        const named = batchInput.tasks.find((t) => t.name === opts.start);
        if (!named) {
          throw new MaestroError(`--start '${opts.start}' does not match any 'name' in the plan`, [
            "The value must match a task's 'name' slot in the same batch",
            "Add a 'name' to the task you want to start, then pass it here",
          ]);
        }
        sessionId = await resolveOwnershipSessionId(opts.session);
        await maybeReleaseStaleOwnedTasks([sessionId]);
      }

      if (opts.dryRun === true) {
        output(isJson, { batchId: batchInput.batchId, taskCount: batchInput.tasks.length, dryRun: true }, (data) => [
          `[ok] Dry run: ${data.taskCount} task(s) validated, nothing written`,
        ]);
        return;
      }

      const result = await planTasks(services.taskStore, batchInput);

      let startedTaskId: string | undefined;
      let startedPatch: { status: Task["status"]; assignee?: string } | undefined;
      if (opts.start !== undefined && sessionId !== undefined) {
        const target = result.created.find((t) => t.name === opts.start);
        if (target) {
          try {
            const { task: updated, autoClaimed } = await updateTask(
              services.taskStore,
              target.id,
              { status: "in_progress" },
              { sessionId },
            );
            startedTaskId = updated.id;
            startedPatch = { status: updated.status, assignee: updated.assignee };
            warnAutoClaimed(updated, autoClaimed);
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            warn(`Could not auto-start ${target.id}: ${message}`);
            warn("Batch was committed. Claim manually if needed: maestro task claim <id> --force");
          }
        }
      }

      const created = startedPatch
        ? result.created.map((t) => (t.name === opts.start ? { ...t, ...startedPatch } : t))
        : result.created;

      output(isJson, { ...result, created, startedTaskId }, (r) => {
        const lines = [`[ok] ${r.created.length} task(s) created`];
        for (const task of r.created) {
          const label = task.name ? `${task.name}  --> ${task.id}` : `  --> ${task.id}`;
          lines.push(`  ${label}`);
        }
        if (r.startedTaskId) {
          lines.push(`[ok] Started: ${r.startedTaskId}`);
        }
        return lines;
      });

      await refreshNowMd();
    });
}

async function readPlanSource(path: string): Promise<string> {
  const content = await readTextOrStdin(path);
  if (content === undefined) {
    throw new MaestroError(`Plan file not found: ${path}`, [
      "Check the path and retry",
      "Use '-' to read plan JSON from stdin",
    ]);
  }
  return content;
}

function registerQuickCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("q <title>")
    .description("Quick capture: create a task and print its id only")
    .option("--type <type>", `Task type (${TASK_TYPES.join("|")})`)
    .option("--priority <n>", "Priority 0-4")
    .option("--labels <labels>", "Comma-separated labels")
    .option("--parent <id>", "Parent task id")
    .option("--blocked-by <ids>", "Comma-separated blocker task ids")
    .option("--json", "Output as JSON")
    .action(async (title: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const input = buildCreateInput(title, {
        type: opts.type,
        priority: opts.priority,
        labels: opts.labels,
        parent: opts.parent,
        blockedBy: opts.blockedBy,
      });
      const task = await createTask(services.taskStore, input);

      await refreshNowMd();

      if (isJson) {
        output(true, { id: task.id }, () => []);
        return;
      }
      console.log(task.id);
    });
}

function registerShowCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("show <id>")
    .description("Show task details")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (isJson) {
        const task = await showTask(services.taskStore, id);
        output(true, task, formatTaskDetail);
        return;
      }

      const view = await buildTaskShowView({
        taskStore: services.taskStore,
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
      }, id);
      output(false, view, formatTaskShowView);
    });
}

function registerListCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("list")
    .description("List tasks with optional filters")
    .option("--status <status>", `Filter by status (${TASK_STATUSES.join("|")})`)
    .option("--priority <n>", "Filter by priority 0-4")
    .option("--type <type>", `Filter by type (${TASK_TYPES.join("|")})`)
    .option("--label <label>", "Filter by label (single)")
    .option("--parent <id>", "Filter by parent task id")
    .option("--assignee <name>", "Filter by assignee")
    .option("--limit <n>", "Maximum number of tasks to return")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const filters: ListTasksFilters = {
        status: parseStatus(opts.status),
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        label: opts.label,
        parentId: opts.parent,
        assignee: opts.assignee,
        limit: parseLimit(opts.limit),
      };

      const tasks = await listTasks(services.taskStore, filters);
      output(isJson, tasks, formatTaskList);
    });
}

function registerUpdateCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("update <id>")
    .description("Update task fields or move task status explicitly")
    .option("--title <title>", "New title")
    .option("--description <text>", "New description")
    .option("--status <status>", `New status (${TASK_STATUSES.join("|")})`)
    .option("--reason <text>", "Completion reason when --status completed")
    .option("--summary <text>", "Receipt summary captured on --status completed (defaults to --reason)")
    .option("--surprise <text>", "Surprise/gotcha captured on --status completed")
    .option("--verified-by <name>", "Verifier captured on --status completed (repeatable)", appendVerifier, [] as string[])
    .option("--strict", "Block completion when the contract verdict is broken")
    .option("--no-contract", "Allow completion without a contract when contracts.default=required")
    .option("--priority <n>", "New priority 0-4")
    .option("--type <type>", `New type (${TASK_TYPES.join("|")})`)
    .option("--parent <id>", "New parent id (empty string clears)")
    .option("--current-state <text>", "Update the resumable current-state summary")
    .option("--next-action <text>", "Update the resumable next action")
    .option("--add-decision <items>", "Comma-separated active decisions to add")
    .option("--remove-decision <items>", "Comma-separated active decisions to remove")
    .option("--force", "Override ownership checks on claimed tasks")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .addOption(new Option("--assignee <name>").hideHelp())
    .option("--add-label <labels>", "Comma-separated labels to add")
    .option("--remove-label <labels>", "Comma-separated labels to remove")
    .addOption(new Option("--claim").hideHelp())
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const previous = await services.taskStore.get(id);
      const continuationEdits = parseContinuationEdits(opts);

      if (opts.assignee !== undefined) {
        throw taskUpdateOwnershipViaClaim();
      }
      if (opts.claim === true) {
        throw taskUpdateClaimViaDedicatedCommand();
      }

      const patch: UpdateTaskInput = {
        title: opts.title,
        description: opts.description,
        status: parseStatus(opts.status),
        reason: opts.reason,
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        parentId: opts.parent,
        addLabels: parseList(opts.addLabel),
        removeLabels: parseList(opts.removeLabel),
        summary: typeof opts.summary === "string" ? opts.summary : undefined,
        surprise: typeof opts.surprise === "string" ? opts.surprise : undefined,
        verifiedBy: Array.isArray(opts.verifiedBy) && opts.verifiedBy.length > 0
          ? opts.verifiedBy as readonly string[]
          : undefined,
      };

      const hasTaskPatch = hasAnyPatchField(patch);
      const hasContinuationPatch = hasContinuationEdits(continuationEdits);
      const noContract = opts.contract === false;

      if (!hasTaskPatch && !hasContinuationPatch) {
        throw new MaestroError("No update specified", [
          "Pass at least one field such as --title, --description, --status, --reason,",
          "--priority, --type, --parent, --current-state, --next-action,",
          "--add-decision, --remove-decision, --add-label, or --remove-label",
        ]);
      }

      const sessionId = await resolveSessionAndReleaseStale(opts.session);
      if (
        previous?.status === "completed"
        && patch.status === "completed"
        && !hasAdditionalCompletedTaskEdits(patch, continuationEdits)
      ) {
        if (emitSilentSuccess(isJson, opts, previous)) return;
        output(isJson, previous, (task) => [
          `[ok] Task updated: ${task.id}`,
          `  Status: ${task.status}`,
          `  Priority: P${task.priority}`,
          ...(task.assignee ? [`  Assignee: ${task.assignee}`] : []),
          ...(task.blockedBy.length > 0 ? [`  Blocked by: ${task.blockedBy.join(", ")}`] : []),
          ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : []),
          ...(task.closeReason ? [`  Reason: ${task.closeReason}`] : []),
        ]);
        return;
      }
      if (previous?.status === "completed" && patch.status === "completed") {
        throw completedTaskUpdateRequiresReopen(id);
      }
      if (patch.status === "completed" && previous) {
        await enforceContractCompletionPolicy(previous, patch, {
          strictFlag: opts.strict === true,
          noContract,
        });
      }
      let updated: Task;
      let autoClaimed = false;

      if (hasTaskPatch) {
        if (previous?.status === "completed" && patch.status === "in_progress") {
          await preflightCompletedTaskRestart(previous, patch, {
            sessionId,
            force: opts.force === true,
          });
          await reopenTaskFlow({
            taskStore: services.taskStore,
            continuationStore: services.taskContinuationStore,
            continuationHistory: services.taskContinuationHistory,
            contractStore: services.contractStore,
          }, id);
        }
        const result = await updateTask(
          services.taskStore,
          id,
          patch,
          {
            sessionId,
            force: opts.force === true,
          },
        );
        updated = result.task;
        autoClaimed = result.autoClaimed;
      } else {
        if (!previous) {
          throw new MaestroError(`Task not found: ${id}`);
        }
        if (previous.status === "completed") {
          throw completedTaskUpdateRequiresReopen(id);
        }
        assertTaskMutationOwnership(previous, { sessionId, force: opts.force === true }, "update");
        updated = previous;
      }

      warnAutoClaimed(updated, autoClaimed);
      if (hasTaskPatch) {
        await maybeCaptureCompletionHint(updated);
      }
      await applyUpdateContinuation(
        {
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        previous,
        updated,
        patch,
        autoClaimed,
        continuationEdits,
      );

      if (emitSilentSuccess(isJson, opts, updated)) return;

      output(isJson, updated, (task) => [
        `[ok] Task updated: ${task.id}`,
        `  Status: ${task.status}`,
        `  Priority: P${task.priority}`,
        ...(task.assignee ? [`  Assignee: ${task.assignee}`] : []),
        ...(task.blockedBy.length > 0 ? [`  Blocked by: ${task.blockedBy.join(", ")}`] : []),
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : []),
        ...(task.closeReason ? [`  Reason: ${task.closeReason}`] : []),
      ]);
    });
}

function registerClaimCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("claim <id>")
    .description("Claim exclusive ownership of a task")
    .option("--force", "Take over a task already claimed by another session")
    .option("--busy-check", "Reject the claim if this session already owns unresolved work")
    .option("--contract-required", "Always print the contract reminder note after claim")
    .option("--no-contract", "Suppress the contract reminder note for this claim")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--stale-after <duration>", "Auto-release a dead owner's stale claim after this idle window (default 4h)")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);
      if (opts.contractRequired === true && opts.contract === false) {
        throw new MaestroError("Choose either --contract-required or --no-contract, not both");
      }
      await maybeReleaseStaleOwnedTasks([sessionId]);
      await maybeReleaseStaleClaim(id, sessionId, opts.staleAfter);
      const previous = await services.taskStore.get(id);

      const claimed = await claimTask(services.taskStore, id, {
        sessionId,
        force: opts.force === true,
        checkBusy: opts.busyCheck === true,
      });
      if (claimed.contractId) {
        await maybeTransferClaimedContractOwnership(claimed.id, sessionId);
      }
      await maybeAttachClaimAnchor(claimed.id);
      await syncTaskContinuation(
        {
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        buildClaimContinuationInput(previous, claimed),
      );

      await refreshNowMd();
      try {
        await maybeWarnMissingContractAfterClaim(claimed, {
          contractRequired: opts.contractRequired === true,
          noContract: opts.contract === false,
        });
      } catch {
        // Contract reminder notes must not block a successful claim.
      }

      if (emitSilentSuccess(isJson, opts, claimed)) return;

      output(isJson, claimed, (task) => [
        `[ok] Task claimed: ${task.id}`,
        `  Assignee: ${task.assignee}`,
        `  Status: ${task.status}`,
      ]);
    });
}

function registerUnclaimCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("unclaim <id>")
    .description("Release task ownership")
    .option("--force", "Release a task owned by another session")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);
      const previous = await services.taskStore.get(id);

      const unclaimed = await unclaimTask(services.taskStore, id, {
        sessionId,
        force: opts.force === true,
      });
      await syncTaskContinuation(
        {
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        {
          task: unclaimed,
          summary: {
            currentState: "Task ownership released back to the queue.",
            activeAgent: null,
          },
          event: {
            kind: "snapshot",
            at: unclaimed.updatedAt,
            summary: previous?.assignee ? `Ownership released from ${previous.assignee}` : "Ownership released",
            currentState: "Task ownership released back to the queue.",
          },
        },
      );

      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, unclaimed)) return;

      output(isJson, unclaimed, (task) => [
        `[ok] Task unclaimed: ${task.id}`,
        `  Status: ${task.status}`,
      ]);
    });
}

function registerReleaseOwnedCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("release-owned <sessionId>")
    .description("Release unresolved tasks owned by a dead or stale session")
    .option("--silent", "Print only '<id> <marker>' per released task (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (sessionId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const trimmedSessionId = sessionId.trim();
      const beforeTasks = await services.taskStore.all();
      const before = new Map(beforeTasks.map((task) => [task.id, task] as const));
      const released = await releaseMatchingOwnedTasks(services.taskStore, beforeTasks, trimmedSessionId);
      await syncRecoveredStaleOwnerTasks(services, before, released);

      await refreshNowMd();

      if (!isJson && resolveSilent(opts)) {
        released.forEach(printSilent);
        return;
      }

      output(isJson, released, (tasks) => {
        if (tasks.length === 0) {
          return [`No unresolved tasks owned by ${trimmedSessionId}`];
        }
        return [
          `[ok] Released ${tasks.length} task(s) owned by ${trimmedSessionId}`,
          ...tasks.map((task) => `  ${task.id} -> ${task.status}`),
        ];
      });
    });
}

function registerReopenCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("reopen <id>")
    .description("Restore a completed task to the pending queue")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const reopened = await reopenTaskFlow({
        taskStore: services.taskStore,
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
        contractStore: services.contractStore,
      }, id);
      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, reopened.task)) return;

      output(isJson, reopened.task, (task) => [
        `[ok] Task reopened: ${task.id}`,
        `  Status: ${task.status}`,
        `  Next: Resume ${task.title}.`,
      ]);
    });
}

function registerDeleteCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("delete <id>")
    .description("Delete a task and clean up its contract and continuation state")
    .option("--force", "Delete a task claimed by another session")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOptionalOwnershipSessionId(opts.session);
      const actor: TaskMutationInput = {
        ...(opts.force === true ? { force: true } : {}),
        ...(sessionId ? { sessionId } : {}),
      };
      const deleted = await deleteTaskFlow({
        taskStore: services.taskStore,
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
        contractStore: services.contractStore,
      }, id, actor);
      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, deleted)) return;

      output(isJson, deleted, (task) => [
        `[ok] Task deleted: ${task.id}`,
        `  Title: ${task.title}`,
      ]);
    });
}

const DEFAULT_PRUNE_KEEP = 500;

function registerPruneCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("prune")
    .description("Bound local per-machine task state (candidates + completed continuations)")
    .option("--keep <n>", `Keep the most recent N entries per kind (default ${DEFAULT_PRUNE_KEEP})`)
    .option("--candidates-only", "Only prune .maestro/tasks/candidates/")
    .option("--continuations-only", "Only prune .maestro/tasks/continuations/completed/")
    .option("--all", "Purge everything in the targeted directories")
    .option("--dry-run", "Report what would be purged without deleting")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      if (opts.candidatesOnly === true && opts.continuationsOnly === true) {
        throw new MaestroError(
          "Choose either --candidates-only or --continuations-only, not both",
        );
      }

      const isJson = resolveJsonFlag(opts, program);
      const keep = parseLimit(opts.keep) ?? DEFAULT_PRUNE_KEEP;
      const kinds: PruneKinds = opts.candidatesOnly === true
        ? "candidates"
        : opts.continuationsOnly === true
          ? "continuations"
          : "both";

      const services = getServices();
      const report = await pruneLocalTaskState(
        {
          candidateStore: services.taskCandidateStore,
          continuationStore: services.taskContinuationStore,
        },
        {
          keep,
          kinds,
          all: opts.all === true,
          dryRun: opts.dryRun === true,
        },
      );

      output(isJson, report, formatPruneReport);
    });
}

function registerBlockCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("block <id> <blockedTaskIds...>")
    .description("Mark this task as blocking the target task ids")
    .option("--force", "Override ownership checks on claimed tasks")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, blockedTaskIds: string[], opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveSessionAndReleaseStale(opts.session);
      const before = await services.taskStore.get(id);
      const updated = await blockTasks(
        services.taskStore,
        id,
        blockedTaskIds,
        {
          sessionId,
          force: opts.force === true,
        },
      );
      const tasksById = new Map((await services.taskStore.all()).map((task) => [task.id, task] as const));
      for (const blockedTaskId of blockedTaskIds) {
        const blockedTask = tasksById.get(blockedTaskId);
        if (!blockedTask) continue;
        await syncTaskContinuation(
          {
            continuationStore: services.taskContinuationStore,
            continuationHistory: services.taskContinuationHistory,
          },
          {
            task: blockedTask,
            summary: {
              currentState: `Blocked by ${id}.`,
              nextAction: `Resolve blocker ${id} before continuing ${blockedTask.title}.`,
            },
            event: {
              kind: "blocker_set",
              at: blockedTask.updatedAt,
              summary: `Blocked by ${id}`,
              blockerTaskIds: blockedTask.blockedBy,
            },
          },
        );
      }
      await syncTaskContinuation(
        {
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        {
          task: updated,
          summary: {
            currentState: `Blocking ${updated.blocks.length} task(s).`,
          },
          event: {
            kind: "snapshot",
            at: updated.updatedAt,
            summary: before?.blocks.length === updated.blocks.length
              ? `Blockers unchanged`
              : `Now blocking ${updated.blocks.join(", ")}`,
            currentState: `Blocking ${updated.blocks.length} task(s).`,
          },
        },
      );

      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, updated)) return;

      output(isJson, updated, (task) => [
        `[ok] Blockers added: ${task.id}`,
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : ["  Blocks: none"]),
      ]);
    });
}

function registerUnblockCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("unblock <id> <blockedTaskIds...>")
    .description("Remove blocker edges from this task to the target task ids")
    .option("--force", "Override ownership checks on claimed tasks")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, blockedTaskIds: string[], opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveSessionAndReleaseStale(opts.session);
      const updated = await unblockTasks(
        services.taskStore,
        id,
        blockedTaskIds,
        {
          sessionId,
          force: opts.force === true,
        },
      );
      const tasksById = new Map((await services.taskStore.all()).map((task) => [task.id, task] as const));
      for (const blockedTaskId of blockedTaskIds) {
        const blockedTask = tasksById.get(blockedTaskId);
        if (!blockedTask) continue;
        await syncTaskContinuation(
          {
            continuationStore: services.taskContinuationStore,
            continuationHistory: services.taskContinuationHistory,
          },
          {
            task: blockedTask,
            summary: blockedTask.blockedBy.length === 0
              ? {
                  currentState: "Blockers cleared and ready to resume.",
                  nextAction: `Resume ${blockedTask.title}.`,
                }
              : undefined,
            event: {
              kind: "blocker_set",
              at: blockedTask.updatedAt,
              summary: blockedTask.blockedBy.length === 0 ? `Blockers cleared` : `Remaining blockers: ${blockedTask.blockedBy.join(", ")}`,
              blockerTaskIds: blockedTask.blockedBy,
            },
          },
        );
      }
      await syncTaskContinuation(
        {
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        {
          task: updated,
          event: {
            kind: "snapshot",
            at: updated.updatedAt,
            summary: updated.blocks.length === 0 ? "No longer blocking other tasks" : `Still blocking ${updated.blocks.join(", ")}`,
            currentState: updated.blocks.length === 0 ? "No longer blocking other tasks." : `Blocking ${updated.blocks.length} task(s).`,
          },
        },
      );

      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, updated)) return;

      output(isJson, updated, (task) => [
        `[ok] Blockers removed: ${task.id}`,
        ...(task.blocks.length > 0 ? [`  Blocks: ${task.blocks.join(", ")}`] : ["  Blocks: none"]),
      ]);
    });
}

function registerLegacyDepsCommand(taskCmd: Command): void {
  const depsCmd = taskCmd
    .command("deps")
    .description("Legacy compatibility shim for renamed blocker commands");

  depsCmd
    .command("add <id> <dependencyIds...>")
    .description("Legacy compatibility shim")
    .action(() => {
      throw taskDependencyCommandsRenamed();
    });

  depsCmd
    .command("remove <id> <dependencyIds...>")
    .description("Legacy compatibility shim")
    .action(() => {
      throw taskDependencyCommandsRenamed();
    });
}

function registerCloseCommand(taskCmd: Command): void {
  taskCmd
    .command("close <id>")
    .description("Legacy compatibility shim; completion moved to task update")
    .action(() => {
      throw taskCompletedViaUpdateStatus();
    });
}

async function resolveOwnershipSessionId(explicitSessionId: string | undefined): Promise<string> {
  if (explicitSessionId !== undefined) {
    const trimmed = explicitSessionId.trim();
    if (trimmed.length === 0) {
      throw new MaestroError("Invalid --session value", [
        "Pass a non-empty session id such as 'codex-1234' or 'operator-recovery'",
      ]);
    }
    return trimmed;
  }

  const services = getServices();
  const session = await services.sessionDetect.detect(process.cwd());
  if (!session) {
    throw new MaestroError("Could not detect current session for task ownership", [
      "Set CODEX_THREAD_ID or run from an agent environment",
      "Or pass --session <id> for an explicit operator or CI override",
    ]);
  }
  return buildTaskOwnerId(session.agent, session.sessionId);
}

async function resolveOptionalOwnershipSessionId(explicitSessionId: string | undefined): Promise<string | undefined> {
  if (explicitSessionId !== undefined) {
    return resolveOwnershipSessionId(explicitSessionId);
  }

  const services = getServices();
  const session = await services.sessionDetect.detect(process.cwd());
  return session ? buildTaskOwnerId(session.agent, session.sessionId) : undefined;
}

async function resolveSessionAndReleaseStale(
  explicitSessionId: string | undefined,
): Promise<string | undefined> {
  const sessionId = await resolveOptionalOwnershipSessionId(explicitSessionId);
  await maybeReleaseStaleOwnedTasks(sessionId ? [sessionId] : []);
  return sessionId;
}

async function preflightCompletedTaskRestart(
  previous: Task,
  patch: UpdateTaskInput,
  actor: TaskMutationInput,
): Promise<void> {
  const services = getServices();
  const reopenedTask = buildPreflightReopenedTask(previous);
  const allTasks = await services.taskStore.all();
  const tasks = indexTasksById(allTasks.map((task) => task.id === reopenedTask.id ? reopenedTask : task));
  assertTaskUpdateAllowed(reopenedTask, patch, tasks, actor);
  await loadContractForReopen(services.contractStore, previous);
}

function buildPreflightReopenedTask(previous: Task): Task {
  return {
    ...previous,
    status: "pending",
    assignee: undefined,
    claimedAt: undefined,
    lastActivityAt: undefined,
    closeReason: undefined,
    receipt: undefined,
    updatedAt: previous.updatedAt,
  };
}

async function releaseMatchingOwnedTasks(
  taskStore: ReturnType<typeof getServices>["taskStore"],
  tasks: readonly Task[],
  sessionId: string,
): Promise<readonly Task[]> {
  const ownerIds = collectReleaseOwnerIds(tasks, sessionId);
  const released: Task[] = [];
  const seen = new Set<string>();

  for (const ownerId of ownerIds) {
    const ownerReleased = await releaseOwnedTasks(taskStore, ownerId);
    for (const task of ownerReleased) {
      if (seen.has(task.id)) {
        continue;
      }
      seen.add(task.id);
      released.push(task);
    }
  }

  return released;
}

function collectReleaseOwnerIds(
  tasks: readonly Task[],
  sessionId: string,
): readonly string[] {
  const ownerIds = new Set<string>([sessionId]);

  for (const task of tasks) {
    if (!task.assignee) {
      continue;
    }
    const parsed = parseTaskOwnerId(task.assignee);
    if (parsed?.sessionId === sessionId) {
      ownerIds.add(task.assignee);
    }
  }

  return [...ownerIds];
}

function warnAutoClaimed(task: Task, autoClaimed: boolean): void {
  if (autoClaimed && task.assignee) {
    warn(`Auto-claimed ${task.id} for session ${task.assignee}`);
  }
}

function buildClaimContinuationInput(previous: Task | undefined, claimed: Task) {
  const previousAgent = deriveAgentFromAssignee(previous?.assignee, previous?.updatedAt ?? claimed.updatedAt);
  const nextAgent = deriveAgentFromAssignee(claimed.assignee, claimed.updatedAt);

  if (previous?.assignee && previous.assignee !== claimed.assignee && nextAgent) {
    return {
      task: claimed,
      summary: {
        currentState: "Task claimed and ready to continue.",
      },
      event: {
        kind: "agent_takeover" as const,
        at: claimed.updatedAt,
        summary: `${nextAgent.type} resumed this task from ${previousAgent?.type ?? previous.assignee}`,
        reason: "claim" as const,
        ...(previousAgent ? { from: previousAgent } : {}),
        to: nextAgent,
      },
    };
  }

  return {
    task: claimed,
    summary: {
      currentState: "Task claimed and ready to start.",
      nextAction: `Start ${claimed.title}.`,
    },
    event: {
      kind: "snapshot" as const,
      at: claimed.updatedAt,
      summary: claimed.assignee ? `Claimed by ${claimed.assignee}` : "Claimed",
      currentState: "Task claimed and ready to start.",
    },
  };
}

function buildUpdateContinuationInput(
  previous: Task | undefined,
  updated: Task,
  patch: UpdateTaskInput,
  autoClaimed: boolean,
  edits: ContinuationEditInput,
  keyDecisions: readonly string[],
  at: string,
) {
  if (updated.status === "completed") {
    const closeSummary = updated.closeReason
      ? `Completed: ${updated.closeReason}`
      : `Completed: ${updated.title}`;
    return {
      task: updated,
      summary: {
        lastActiveAt: at,
        currentState: closeSummary,
        nextAction: "Review the completed task and decide whether follow-up work is needed.",
        keyDecisions,
        activeAgent: null,
      },
      events: [
        {
          kind: "task_completed" as const,
          at,
          summary: closeSummary,
          ...(updated.closeReason ? { reason: updated.closeReason } : {}),
        },
      ],
    };
  }

  if (patch.status === "in_progress" && previous?.status !== "in_progress") {
    const currentState = edits.currentState ?? (updated.description?.trim().length
      ? updated.description!.trim()
      : `Working on ${updated.title}`);
    const nextAction = edits.nextAction ?? currentState;
    return {
      task: updated,
      summary: {
        lastActiveAt: at,
        currentState,
        nextAction,
        keyDecisions,
      },
      events: [
        {
          kind: "snapshot" as const,
          at,
          summary: autoClaimed ? "Auto-claimed and started work" : "Started work",
          currentState,
        },
        ...(edits.nextAction
          ? [{
              kind: "next_action_set" as const,
              at,
              summary: "Next action updated",
              nextAction,
            }]
          : []),
        ...buildDecisionEvents(edits, at),
      ],
    };
  }

  const currentState = edits.currentState ?? (patch.description ?? patch.title
    ? (updated.description?.trim().length
        ? updated.description!.trim()
        : `Working on ${updated.title}`)
    : undefined);
  const nextAction = edits.nextAction;
  const updateSummary = currentState
    ? (patch.description ?? patch.title ? "Task summary updated" : "Progress snapshot updated")
    : "Task metadata updated";

  return {
    task: updated,
    summary: {
      lastActiveAt: at,
      ...(currentState !== undefined ? { currentState } : {}),
      ...(nextAction !== undefined ? { nextAction } : {}),
      keyDecisions,
    },
    events: [
      ...(currentState !== undefined
        ? [{
            kind: "snapshot" as const,
            at,
            summary: updateSummary,
            currentState,
          }]
        : []),
      ...(nextAction !== undefined
        ? [{
            kind: "next_action_set" as const,
            at,
            summary: "Next action updated",
            nextAction,
          }]
        : []),
      ...buildDecisionEvents(edits, at),
    ],
  };
}

async function applyUpdateContinuation(
  deps: Parameters<typeof syncTaskContinuation>[0],
  previous: Task | undefined,
  updated: Task,
  patch: UpdateTaskInput,
  autoClaimed: boolean,
  edits: ContinuationEditInput,
): Promise<void> {
  const at = patch.status === undefined && !hasAnyPatchField(patch)
    ? new Date().toISOString()
    : updated.updatedAt;
  const existing = await loadTaskContinuationSummary(deps.continuationStore, updated.id);
  const keyDecisions = mergeDecisionEdits(existing?.keyDecisions ?? [], edits);
  const input = buildUpdateContinuationInput(previous, updated, patch, autoClaimed, edits, keyDecisions, at);
  const summary = buildTaskContinuationSummary(updated, existing, input.summary);

  if (updated.status === "completed") {
    await deps.continuationStore.archiveCompleted(summary);
  } else {
    await deps.continuationStore.upsertActive(summary);
  }

  for (const event of input.events) {
    await deps.continuationHistory.append(updated.id, event);
  }

  await refreshNowMd();
  if (updated.status === "completed" && updated.contractId) {
    await maybeFinalizeTaskContract(updated);
  }
}

function parseContinuationEdits(opts: {
  currentState?: unknown;
  nextAction?: unknown;
  addDecision?: unknown;
  removeDecision?: unknown;
}): ContinuationEditInput {
  return {
    ...(typeof opts.currentState === "string" && opts.currentState.trim().length > 0
      ? { currentState: opts.currentState.trim() }
      : {}),
    ...(typeof opts.nextAction === "string" && opts.nextAction.trim().length > 0
      ? { nextAction: opts.nextAction.trim() }
      : {}),
    addDecisions: parseList(typeof opts.addDecision === "string" ? opts.addDecision : undefined) ?? [],
    removeDecisions: parseList(typeof opts.removeDecision === "string" ? opts.removeDecision : undefined) ?? [],
  };
}

function hasContinuationEdits(edits: ContinuationEditInput): boolean {
  return edits.currentState !== undefined
    || edits.nextAction !== undefined
    || edits.addDecisions.length > 0
    || edits.removeDecisions.length > 0;
}

function mergeDecisionEdits(
  existing: readonly string[],
  edits: ContinuationEditInput,
): readonly string[] {
  const next = new Set(existing);
  for (const decision of edits.addDecisions) {
    next.add(decision);
  }
  for (const decision of edits.removeDecisions) {
    next.delete(decision);
  }
  return [...next];
}

function buildDecisionEvents(edits: ContinuationEditInput, at: string) {
  return [
    ...edits.addDecisions.map((decision) => ({
      kind: "decision" as const,
      at,
      summary: `Decision added: ${decision}`,
      decision,
      active: true,
    })),
    ...edits.removeDecisions.map((decision) => ({
      kind: "decision" as const,
      at,
      summary: `Decision removed: ${decision}`,
      decision,
      active: false,
    })),
  ];
}

function registerReadyCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("ready")
    .description("List actionable pending tasks with no unresolved blockers")
    .option("--limit <n>", "Maximum tasks to return (default 20, 0 = unlimited)")
    .option("--label <label>", "Filter by label")
    .option("--priority <n>", "Filter by priority 0-4")
    .option("--type <type>", `Filter by type (${TASK_TYPES.join("|")})`)
    .option("--assignee <name>", "Filter by assignee")
    .option("--unassigned", "Only include unassigned tasks")
    .option("--no-hints", "Disable lesson hints surfaced from past completed tasks")
    .option("--compact", "Output compact JSON envelope for agents/scripts (use with --json)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      if (opts.compact === true && !isJson) {
        throw new MaestroError("Invalid flag combination: --compact requires --json", [
          "Pass '--json --compact' to request the compact machine-readable envelope",
        ]);
      }
      const useCompactJson = opts.compact === true;
      const showHints = opts.hints !== false;
      await maybeReleaseStaleOwnedTasks();

      const filters: ReadyTasksFilters = {
        limit: parseLimit(opts.limit),
        label: opts.label,
        priority: parsePriority(opts.priority),
        type: parseType(opts.type),
        assignee: opts.assignee,
        unassigned: opts.unassigned === true,
      };

      if (useCompactJson) {
        const page = await readyTaskPage(
          services.taskStore,
          filters,
        );
        output(true, buildCompactReadyTaskPayload(page), () => []);
        return;
      }

      const briefings = await readyTasks(
        services.taskStore,
        filters,
        new Date(),
        showHints ? services.taskCandidateStore : undefined,
      );
      output(isJson, briefings, formatTaskBriefingList);
    });
}

async function maybeReleaseStaleOwnedTasks(skipAssignees: readonly string[] = []): Promise<void> {
  const services = getServices();
  const tasks = await services.taskStore.all();
  const before = new Map(tasks.map((task) => [task.id, task] as const));
  const skipSet = new Set(skipAssignees);
  const tasksByAssignee = new Map<string, Task[]>();
  let refreshedNowMd = false;

  for (const task of tasks) {
    if (!task.assignee || task.status === "completed") {
      continue;
    }
    if (skipSet.has(task.assignee)) {
      continue;
    }
    const parsed = parseTaskOwnerId(task.assignee);
    if (!parsed) {
      continue;
    }
    const grouped = tasksByAssignee.get(task.assignee);
    if (grouped) {
      grouped.push(task);
    } else {
      tasksByAssignee.set(task.assignee, [task]);
    }
  }

  const staleOwners = await collectStaleOwners(services, [...tasksByAssignee.keys()]);
  for (const assignee of staleOwners) {
    const ownedTasks = tasksByAssignee.get(assignee) ?? [];
    try {
      await assertStaleContractsReclaimable(ownedTasks);
      const released = await releaseOwnedTasks(services.taskStore, assignee);
      await syncRecoveredStaleOwnerTasks(services, before, released);
      if (released.length > 0) {
        refreshedNowMd = true;
        warn(`[ok] Released ${released.length} stale task(s) owned by ${assignee}`);
      }
    } catch (error) {
      if (error instanceof MaestroError && error.message.includes("stale reclaim is blocked by contract policy")) {
        warn(error.message);
        continue;
      }
      const message = error instanceof Error ? error.message : String(error);
      warn(`Stale task recovery failed for ${assignee}: ${message}`);
    }
  }

  if (refreshedNowMd) {
    await refreshNowMd();
  }
}

const STALE_OWNER_LOOKUP_CONCURRENCY = 8;

async function collectStaleOwners(
  services: ReturnType<typeof getServices>,
  assignees: readonly string[],
): Promise<readonly string[]> {
  const staleOwners = new Set<string>();

  for (let index = 0; index < assignees.length; index += STALE_OWNER_LOOKUP_CONCURRENCY) {
    const chunk = assignees.slice(index, index + STALE_OWNER_LOOKUP_CONCURRENCY);
    const statuses = await Promise.all(chunk.map(async (assignee) => {
      const parsed = parseTaskOwnerId(assignee);
      if (!parsed) {
        return undefined;
      }
      const session = await services.sessionDetect.lookup(parsed.agent, parsed.sessionId);
      return session ? undefined : assignee;
    }));

    for (const assignee of statuses) {
      if (assignee) {
        staleOwners.add(assignee);
      }
    }
  }

  return [...staleOwners];
}

async function syncRecoveredStaleOwnerTasks(
  services: ReturnType<typeof getServices>,
  before: ReadonlyMap<string, Task>,
  released: readonly Task[],
): Promise<void> {
  for (const task of released) {
    const previous = before.get(task.id);
    await syncTaskContinuation(
      {
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
      },
      {
        task,
        summary: {
          currentState: "Task ownership released back to the queue.",
          activeAgent: null,
        },
        event: {
          kind: "snapshot",
          at: task.updatedAt,
          summary: previous?.assignee ? `Recovered from stale owner ${previous.assignee}` : "Recovered from stale owner",
          currentState: "Task ownership released back to the queue.",
        },
      },
    );
  }
}

async function maybeCaptureCompletionHint(task: Task): Promise<void> {
  if (task.status !== "completed") {
    return;
  }

  const services = getServices();
  try {
    await captureTaskCandidate(services.taskCandidateStore, task);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warn(`Task ${task.id} completed, but hint capture failed: ${message}`);
  }
}

async function maybeAttachClaimAnchor(taskId: string): Promise<void> {
  try {
    const services = getServices();
    const claimedAtCommit = await services.gitAnchor.resolveHeadCommit(process.cwd());
    if (!claimedAtCommit) {
      return;
    }
    await syncTaskMetadata(services.taskStore, taskId, { claimedAtCommit });
  } catch {
    // Claim anchor is best-effort and should not block ownership.
  }
}

async function maybeWarnMissingContractAfterClaim(
  task: Task,
  opts: {
    readonly contractRequired: boolean;
    readonly noContract: boolean;
  },
): Promise<void> {
  if (task.contractId || opts.noContract) {
    return;
  }

  const services = getServices();
  const config = await services.config.load(process.cwd());
  const policy = opts.contractRequired
    ? "required"
    : (config.contracts?.default ?? "prompt");

  if (policy === "optional") {
    return;
  }
  if (policy === "prompt" && !process.stderr.isTTY) {
    return;
  }

  const message = policy === "required"
    ? `Task ${task.id} requires a task contract before substantial work. Create and lock one: maestro task contract new ${task.id} && maestro task contract lock ${task.id}`
    : `Consider creating a task contract before you start: maestro task contract new ${task.id} && maestro task contract lock ${task.id}`;
  warn(message);
}

async function enforceContractCompletionPolicy(
  task: Task,
  patch: UpdateTaskInput,
  opts: { readonly strictFlag: boolean; readonly noContract: boolean },
): Promise<void> {
  const services = getServices();
  const config = await services.config.load(process.cwd());

  if (!task.contractId) {
    if (!opts.noContract && config.contracts?.default === "required") {
      throw new MaestroError(`Task ${task.id} requires a locked contract before completion`, [
        `Create one: maestro task contract new ${task.id}`,
        `Or bypass the requirement for this completion only: maestro task update ${task.id} --status completed --no-contract ...`,
      ]);
    }
    return;
  }

  if (opts.noContract) {
    throw new MaestroError(`Task ${task.id} already has contract ${task.contractId}; --no-contract cannot ignore it`, [
      "Drop --no-contract and either lock the contract or discard it first",
    ]);
  }

  const contract = await services.contractStore.get(task.contractId);
  if (!contract) {
    throw new MaestroError(`Contract ${task.contractId} not found for task ${task.id}`, [
      "Inspect .maestro/tasks/contracts/ for corruption or stale state",
    ]);
  }
  if (contract.status === "draft") {
    throw new MaestroError(`Contract ${contract.id} is still draft`, [
      `Lock it first: maestro task contract lock ${contract.id}`,
    ]);
  }
  if (contract.status === "discarded" || contract.status === "fulfilled" || contract.status === "broken") {
    return;
  }

  const strict = opts.strictFlag || contract.configSnapshot.strict;
  if (!strict) {
    return;
  }

  const preview = await computeContractVerdictForTask(
    services.contractStore,
    services.gitAnchor,
    contract,
    {
      assignee: task.assignee,
      receipt: previewTaskReceipt(task, patch),
      updatedAt: new Date().toISOString(),
    },
    undefined,
    await services.gitAnchor.resolveRepoRoot(process.cwd()),
  );

  if (!preview.verdict.fulfilled) {
    throw new MaestroError(`Contract ${contract.id} is broken and strict mode refused completion`, [
      formatVerdictHint(preview.verdict),
      `Inspect the contract: maestro task contract show ${contract.id} --json`,
    ]);
  }
}

function previewTaskReceipt(task: Task, patch: UpdateTaskInput): TaskReceipt | undefined {
  return buildTaskReceipt(task.receipt, {
    nextStatus: "completed",
    capturedAt: new Date().toISOString(),
    summary: patch.summary,
    surprise: patch.surprise,
    verifiedBy: patch.verifiedBy,
    reasonFallback: patch.reason,
  });
}

function hasAdditionalCompletedTaskEdits(
  patch: UpdateTaskInput,
  continuationEdits: ContinuationEditInput,
): boolean {
  return (
    patch.title !== undefined
    || patch.description !== undefined
    || patch.reason !== undefined
    || patch.priority !== undefined
    || patch.type !== undefined
    || patch.parentId !== undefined
    || (patch.addLabels !== undefined && patch.addLabels.length > 0)
    || (patch.removeLabels !== undefined && patch.removeLabels.length > 0)
    || patch.summary !== undefined
    || patch.surprise !== undefined
    || (patch.verifiedBy !== undefined && patch.verifiedBy.length > 0)
    || hasContinuationEdits(continuationEdits)
  );
}

function completedTaskUpdateRequiresReopen(id: string): MaestroError {
  return new MaestroError(`Task ${id} is already completed and cannot be updated`, [
    "Reopen the task first if you want to revise its receipt or continuation",
  ]);
}

function formatVerdictHint(verdict: {
  readonly outOfScopeFiles: readonly string[];
  readonly forbiddenTouched: readonly string[];
  readonly unmetCriteria: readonly Array<{ readonly text: string }>;
  readonly capExceeded?: { readonly actual: number; readonly cap: number };
}): string {
  if (verdict.outOfScopeFiles.length > 0) {
    return `Out of scope: ${verdict.outOfScopeFiles.join(", ")}`;
  }
  if (verdict.forbiddenTouched.length > 0) {
    return `Forbidden files touched: ${verdict.forbiddenTouched.join(", ")}`;
  }
  if (verdict.unmetCriteria.length > 0) {
    return `Unmet criteria: ${verdict.unmetCriteria.map((item) => item.text).join(" | ")}`;
  }
  if (verdict.capExceeded) {
    return `Touched ${verdict.capExceeded.actual} files, exceeding the cap of ${verdict.capExceeded.cap}`;
  }
  return "Inspect the stored verdict for details.";
}

async function maybeFinalizeTaskContract(task: Task): Promise<void> {
  try {
    const services = getServices();
    await closeContractForTask(
      services.contractStore,
      services.gitAnchor,
      task,
      await services.gitAnchor.resolveRepoRoot(process.cwd()),
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warn(`Task ${task.id} completed, but contract close failed: ${message}`);
  }
}

async function maybeTransferClaimedContractOwnership(
  taskId: string,
  newActor: string,
  reason: "claim_reclaim" | "handoff_pickup" = "claim_reclaim",
): Promise<void> {
  try {
    const services = getServices();
    await transferContractOwnership(services.contractStore, taskId, newActor, reason);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warn(`Task ${taskId} kept its new owner, but contract ownership transfer failed: ${message}`);
  }
}

async function refreshNowMd(): Promise<void> {
  try {
    const services = getServices();
    const tasks = await services.taskStore.all();
    await services.taskNowMdWriter.write(tasks);
  } catch {
    // NOW.md is a derived view; never block a mutation on it
  }
}

const DEFAULT_STALE_AFTER_MS = 4 * 60 * 60 * 1000;

async function maybeReleaseStaleClaim(
  taskId: string,
  newSessionId: string,
  staleAfterRaw: unknown,
): Promise<void> {
  const services = getServices();
  const task = await services.taskStore.get(taskId);
  if (!task || !task.assignee || task.assignee === newSessionId) {
    return;
  }

  const thresholdMs = typeof staleAfterRaw === "string"
    ? parseDuration(staleAfterRaw, "--stale-after")
    : DEFAULT_STALE_AFTER_MS;

  const last = lastStaleClaimActivityMs(task);
  if (last === undefined) {
    return;
  }
  const idleMs = Date.now() - last;
  if (idleMs < thresholdMs) {
    return;
  }

  const parsed = parseTaskOwnerId(task.assignee);
  if (!parsed) {
    return;
  }
  const session = await services.sessionDetect.lookup(parsed.agent, parsed.sessionId);
  if (session) {
    return;
  }

  const ownedTasks = (await services.taskStore.all()).filter((candidate) =>
    candidate.assignee === task.assignee && candidate.status !== "completed"
  );
  await assertStaleContractsReclaimable(ownedTasks);
  const before = new Map(ownedTasks.map((ownedTask) => [ownedTask.id, ownedTask] as const));
  const released = await releaseOwnedTasks(services.taskStore, task.assignee);
  await syncRecoveredStaleOwnerTasks(services, before, released);
  if (released.length > 0) {
    warn(`Released stale-claim (auto-released) on ${task.id} from ${task.assignee}`);
  }
}

function lastStaleClaimActivityMs(task: Pick<Task, "lastActivityAt" | "updatedAt">): number | undefined {
  const preferred = task.lastActivityAt ? Date.parse(task.lastActivityAt) : Number.NaN;
  if (Number.isFinite(preferred)) {
    return preferred;
  }

  const fallback = Date.parse(task.updatedAt);
  return Number.isFinite(fallback) ? fallback : undefined;
}

async function assertStaleContractsReclaimable(tasks: readonly Task[]): Promise<void> {
  const services = getServices();
  for (const task of tasks) {
    if (!task.contractId) {
      continue;
    }

    const contract = await services.contractStore.get(task.contractId);
    if (!contract || (contract.status !== "locked" && contract.status !== "amended")) {
      continue;
    }
    if (contract.configSnapshot.staleReclaimContractPolicy !== "block") {
      continue;
    }

    throw new MaestroError(
      `Task ${task.id} has active contract ${contract.id}; stale reclaim is blocked by contract policy`,
      [
        `Inspect the contract first: maestro task contract show ${contract.id}`,
        `Release the stale owner manually once reviewed: maestro task release-owned ${task.assignee}`,
      ],
    );
  }
}

function appendVerifier(value: string, previous: string[]): string[] {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return previous;
  }
  return [...previous, trimmed];
}

function resolveSilent(opts: { silent?: unknown }): boolean {
  return resolveTaskSilentMode(opts);
}

const STATUS_MARKER: Record<Task["status"], string> = {
  pending: ".",
  in_progress: ">",
  completed: "x",
};

function printSilent(task: Task): void {
  console.log(`${task.id} ${STATUS_MARKER[task.status]}`);
}

function emitSilentSuccess(
  isJson: boolean,
  opts: { silent?: unknown },
  task: Task,
): boolean {
  if (isJson || !resolveSilent(opts)) return false;
  printSilent(task);
  return true;
}

function registerSimilarCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("similar <id>")
    .description("Show past tasks with keyword overlap across title, completion reason, receipt text, and linked contract text")
    .option("--limit <n>", "Maximum results (default 5, 0 = unlimited)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const limit = opts.limit === undefined ? 5 : parseLimit(opts.limit) ?? 5;

      const matches = await findSimilarTasks(services.taskStore, id, limit, services.contractStore);

      output(isJson, matches, (results) => {
        if (results.length === 0) {
          return ["No similar tasks found"];
        }
        const lines: string[] = [`${results.length} similar task(s)`, ""];
        for (const match of results) {
          const { task } = match;
          const title = task.title.length > 40 ? `${task.title.slice(0, 37)}...` : task.title;
          lines.push(`${task.id}  ${task.status.padEnd(12)}  x${match.overlap}  ${title}`);
          if (task.receipt?.summary) {
            const summary = truncate(task.receipt.summary, 80);
            lines.push(`  summary: ${summary}`);
          }
          if (task.receipt?.surprise) {
            const surprise = truncate(task.receipt.surprise, 80);
            lines.push(`  surprise: ${surprise}`);
          }
        }
        return lines;
      });
    });
}

function registerMineCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("mine")
    .description("List tasks owned by the current session")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--status <status>", `Filter by status (${TASK_STATUSES.join("|")})`)
    .option("--limit <n>", "Maximum tasks to return")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);

      const filters: ListTasksFilters = {
        status: parseStatus(opts.status),
        limit: parseLimit(opts.limit),
        assignee: sessionId,
      };
      const tasks = await listTasks(services.taskStore, filters);
      output(isJson, tasks, formatTaskList);
    });
}

function registerStuckCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("stuck")
    .description("List in_progress tasks with no activity for a while")
    .option("--older-than <duration>", "Inactivity threshold, e.g. 4h, 30m, 2d (default 4h)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const thresholdMs = typeof opts.olderThan === "string"
        ? parseDuration(opts.olderThan, "--older-than")
        : STUCK_THRESHOLD_MS;
      const now = new Date();

      const all = await services.taskStore.all();
      const stuck = all
        .filter((task) => task.status === "in_progress" && isStuckTask(task, now, thresholdMs))
        .slice()
        .sort((a, b) => (a.lastActivityAt ?? a.updatedAt).localeCompare(b.lastActivityAt ?? b.updatedAt));

      output(isJson, stuck, formatTaskList);
    });
}

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max - 3)}...` : text;
}

function registerHeartbeatCommand(taskCmd: Command, program: Command): void {
  taskCmd
    .command("heartbeat <id>")
    .description("Bump the task's lastActivityAt timestamp without any other state change")
    .option("--force", "Heartbeat a task owned by another session")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> <marker>' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const sessionId = await resolveOwnershipSessionId(opts.session);

      const task = await heartbeatTask(services.taskStore, id, sessionId, {
        force: opts.force === true,
      });

      await refreshNowMd();

      if (emitSilentSuccess(isJson, opts, task)) return;

      output(isJson, task, (t) => [
        `[ok] Task heartbeat: ${t.id}`,
        `  Last activity: ${t.lastActivityAt ?? "n/a"}`,
      ]);
    });
}
