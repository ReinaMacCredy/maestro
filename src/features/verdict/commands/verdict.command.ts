import os from "node:os";
import type { Command } from "commander";
import { resolveJsonFlag, output } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { EvidenceStorePort, VerdictOverridePayload } from "@/features/evidence/index.js";
import { loadOwnersFromBase } from "@/features/policy/index.js";
import type { Owners } from "@/features/policy/index.js";
import { resolveDefaultBase } from "@/shared/lib/git-base.js";
import type { Verdict } from "../domain/types.js";
import { exitCodeForDecision, printVerdict } from "../presentation.js";
import { requestVerdict } from "../usecases/request-verdict.usecase.js";

async function loadVerdictOverrides(
  evidenceStore: EvidenceStorePort,
  taskId: string,
  verdictId: string,
): Promise<readonly VerdictOverridePayload[]> {
  const rows = await evidenceStore.list({ task_id: taskId, kind: "verdict-override" });
  return rows
    .filter((r) => (r.payload as VerdictOverridePayload).verdictId === verdictId)
    .map((r) => r.payload as VerdictOverridePayload);
}

interface VerdictCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "contractVersionStore"
    | "contractStore"
    | "runStateStore"
    | "evidenceStore"
    | "getEffectiveRiskPolicy"
    | "getEffectiveAutopilotPolicy"
    | "getEffectiveReleasePolicy"
    | "computeRisk"
    | "deriveRiskClassFromDiff"
    | "runTrustVerifier"
    | "gitAnchor"
    | "projectRoot"
  >;
  readonly getUsername?: () => string;
  readonly loadOwnersFromBase?: (base: string, projectRoot: string) => Owners;
  readonly recordEvidence?: typeof recordEvidence;
}

export function registerVerdictCommand(
  program: Command,
  deps: VerdictCommandDeps = {
    getServices,
    getUsername: () => os.userInfo().username,
    loadOwnersFromBase,
    recordEvidence,
  },
): void {
  const verdictCmd = program
    .command("verdict")
    .description("Show or request a Verdict for a task");

  verdictCmd
    .command("show")
    .description("Show the current verdict for a task")
    .requiredOption("--task <id>", "Task ID")
    .option("--version <verdictId>", "Show a specific verdict by ID (default: latest)")
    .option("--latest", "Show the latest verdict (default)")
    // --pr is a query-time filter: when provided alongside --task, the latest
    // verdict for that task is filtered by tree SHA + PR number. The tree SHA
    // is resolved from the current HEAD, so this reflects the current worktree
    // content. Requires --task to scope the search.
    .option("--pr <number>", "Filter by PR number (finds verdict by current HEAD tree SHA)", parseInt)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      // --pr: resolve current tree SHA and find matching verdicts
      if (typeof opts.pr === "number") {
        const treeSha = await services.gitAnchor.resolveTreeSha(process.cwd());
        const matches = await services.verdictStore.findByTreeSha(treeSha);
        const filtered = matches.filter(
          (v) => v.subject?.pr === opts.pr && v.taskId === taskId,
        );
        if (filtered.length === 0) {
          console.log(`No verdict found for PR ${opts.pr} at tree SHA ${treeSha}`);
          return;
        }
        // Return the latest match (highest computedAt)
        const verdict = filtered[filtered.length - 1]!;
        if (isJson) {
          console.log(JSON.stringify(verdict, null, 2));
        } else {
          const overrides = await loadVerdictOverrides(
            services.evidenceStore,
            taskId,
            verdict.id,
          );
          printVerdict(verdict, overrides);
        }
        return;
      }

      let verdict: Verdict | undefined;

      if (typeof opts.version === "string" && opts.version.length > 0) {
        verdict = await services.verdictStore.readVersion(taskId, opts.version);
        if (verdict === undefined) {
          throw new MaestroError(`Verdict ${opts.version} not found for task ${taskId}`, [
            "Run 'maestro verdict show --task <id>' (without --version) to see the latest",
          ]);
        }
      } else {
        verdict = await services.verdictStore.readLatest(taskId);
        if (verdict === undefined) {
          console.log("No verdict yet. Run 'maestro verdict request --task <id>' to generate one.");
          return;
        }
      }

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        const overrides = await loadVerdictOverrides(
          services.evidenceStore,
          taskId,
          verdict.id,
        );
        printVerdict(verdict, overrides);
      }
    });

  verdictCmd
    .command("request")
    .description("Compute a new Verdict for a task and persist it")
    .requiredOption("--task <id>", "Task ID")
    .option("--base <ref>", "Base git ref for the diff (default: merge-base with main or upstream)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      const verdict = await requestVerdict(
        { taskId, base: typeof opts.base === "string" ? opts.base : undefined },
        {
          contractVersionStore: services.contractVersionStore,
          contractStore: services.contractStore,
          runStateStore: services.runStateStore,
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
          getEffectiveRiskPolicy: services.getEffectiveRiskPolicy,
          getEffectiveAutopilotPolicy: services.getEffectiveAutopilotPolicy,
          getEffectiveReleasePolicy: services.getEffectiveReleasePolicy,
          riskServices: {
            computeRisk: services.computeRisk,
            deriveRiskClassFromDiff: services.deriveRiskClassFromDiff,
          },
          runTrustVerifier: services.runTrustVerifier,
          gitAnchor: services.gitAnchor,
          projectRoot: services.projectRoot,
        },
      );

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        printVerdict(verdict);
      }

      const exitCode = exitCodeForDecision(verdict.decision);
      if (exitCode !== 0) {
        process.exit(exitCode);
      }
    });

  verdictCmd
    .command("override")
    .description("Record a verdict override with audit trail (requires sensitive_waiver authorization)")
    .addHelpText(
      "after",
      `
Authorization: the invoking user (whoami) must be listed in owners.yaml
under 'sensitive_waiver'. owners.yaml is loaded from the BASE branch, not
the PR head, so self-promotion on the PR branch is rejected (Rule 12).

The original Verdict is NOT rewritten. The override is recorded as an
append-only Evidence row at witness level 'agent-claimed-and-not-reproducible'.
CI will still reflect the original verdict conclusion (a BLOCKed PR
remains blocked), but the override is surfaced in the PR check summary.

Examples:
  maestro verdict override --task tsk-aaaaaa --pr 42 \\
    --reason "Emergency hotfix, approved by on-call lead"
  maestro verdict override --task tsk-aaaaaa --pr 42 \\
    --verdict vrd-bbbbbb --reason "Manual sign-off after review"
`,
    )
    .requiredOption("--task <id>", "Task ID")
    .requiredOption("--pr <number>", "PR number this override applies to", parseInt)
    .requiredOption("--reason <text>", "Reason for the override (free text, audit trail)")
    .option("--verdict <id>", "Verdict ID to override (default: latest for the task)")
    .option("--base <ref>", "Base git ref for loading owners.yaml (default: merge-base with upstream or main)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;
      const reason: string = opts.reason;
      const base: string = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : await resolveDefaultBase();

      // Load owners from base branch (Rule 12 — not from PR head)
      const loadOwnersFn = deps.loadOwnersFromBase ?? loadOwnersFromBase;
      const owners = loadOwnersFn(base, services.projectRoot);

      // Authorization check: invoking user must be in sensitive_waiver
      const getUser = deps.getUsername ?? (() => os.userInfo().username);
      const invoker = getUser();
      if (!owners.sensitiveWaivers.includes(invoker)) {
        console.error(`not-authorized: ${invoker} is not in owners.yaml sensitive_waiver (loaded from ${base})`);
        process.exit(1);
      }

      // Resolve verdict ID: use supplied --verdict or fall back to latest
      let verdictId: string;
      if (typeof opts.verdict === "string" && opts.verdict.length > 0) {
        verdictId = opts.verdict;
      } else {
        const latest = await services.verdictStore.readLatest(taskId);
        if (latest === undefined) {
          throw new MaestroError(`No verdict found for task ${taskId}`, [
            "Run 'maestro verdict request --task <id>' first",
          ]);
        }
        verdictId = latest.id;
      }

      const payload: VerdictOverridePayload = {
        verdictId,
        overriddenBy: invoker,
        reason,
      };

      const recordFn = deps.recordEvidence ?? recordEvidence;
      const row = await recordFn(services.evidenceStore, {
        task_id: taskId,
        kind: "verdict-override",
        payload,
        witness_level: "agent-claimed-and-not-reproducible",
      });

      output(isJson, row, (r) => [
        `[ok] Verdict override recorded: ${r.id}`,
        `  Task:      ${r.task_id}`,
        `  Verdict:   ${payload.verdictId}`,
        `  By:        ${payload.overriddenBy}`,
        `  Reason:    ${payload.reason}`,
        `  Witness:   ${r.witness_level}`,
        `  Created:   ${r.created_at}`,
        "",
        "Note: the original verdict conclusion is unchanged. This override",
        "is an audit record only. CI gate status is not affected.",
      ]);
    });
}
