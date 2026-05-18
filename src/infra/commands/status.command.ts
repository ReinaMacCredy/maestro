import type { Command } from "commander";
import { type Services } from "@/services.js";
import { buildStatusReport } from "@/infra/usecases/build-status-report.usecase.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import type { SetupCheckEntry, SetupCheckReport } from "@/service/setup-check.usecase.js";
import type {
  MissionGroup,
  StatusReport,
  TaskSignal,
} from "@/infra/domain/status-types.js";

interface StatusCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "taskStore"
    | "featureMissionStore"
    | "verdictStore"
    | "evidenceStore"
    | "handoffEmitter"
  >;
}

export function registerStatusCommand(
  program: Command,
  deps: StatusCommandDeps,
): void {
  program
    .command("status")
    .description("Show current maestro state (cold-start view)")
    .option("--json", "Output as JSON")
    .option("--terse", "Collapse maestro_health to failing rows and omit recent transitions (plain output only)")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const terse = Boolean(opts.terse);
      if (isJson && terse) {
        warn("--terse is ignored when --json is set");
      }
      const effectiveTerse = terse && !isJson;

      let report: StatusReport;
      try {
        report = await buildStatusReport({
          taskStore: services.taskStore,
          featureMissionStore: services.featureMissionStore,
          verdictStore: services.verdictStore,
          evidenceStore: services.evidenceStore,
          handoffEmitter: services.handoffEmitter,
          projectDir: process.cwd(),
          terse: effectiveTerse,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error(message);
        process.exitCode = 1;
        return;
      }

      output(isJson, report, (r) => renderPlain(r, effectiveTerse));
    });
}

function renderPlain(report: StatusReport, terse: boolean): string[] {
  const lines: string[] = [];

  lines.push("> Maestro health");
  for (const entry of healthEntries(report.maestro_health)) {
    lines.push(`  ${glyph(entry.status)} ${entry.path}${entry.detail ? ` -- ${entry.detail}` : ""}`);
  }
  if (terse && healthEntries(report.maestro_health).length === 0) {
    lines.push("  [ok] no failing health rows");
  }

  lines.push("", "> Project verified state");
  const { latest_verdict, stuck_verifying_count, stale_handoff_count } = report.project_state;
  if (latest_verdict) {
    lines.push(
      `  [ok] last verdict: ${latest_verdict.decision} (${latest_verdict.taskId} @ ${latest_verdict.computedAt})`,
    );
  } else {
    lines.push("  [--] no verdict yet -- run 'maestro task verify <id>'");
  }
  lines.push(
    stuck_verifying_count > 0
      ? `  [!] ${stuck_verifying_count} task(s) in 'verifying' >24h`
      : "  [ok] no stuck-verifying tasks",
  );
  lines.push(
    stale_handoff_count > 0
      ? `  [!] ${stale_handoff_count} stale handoff(s) (no pickup >24h)`
      : "  [ok] no stale handoffs",
  );

  lines.push("", "> Active missions");
  if (report.missions.length === 0) {
    lines.push("  [--] no active missions -- 'maestro mission new'");
  } else {
    for (const group of report.missions) {
      lines.push(`  ${formatMissionHeader(group)}`);
      for (const t of group.tasks) {
        lines.push(`    ${formatTaskLine(t.task.id, t.task.slug, t.task.state, t.signal)}`);
      }
      if (group.tasks.length === 0) {
        lines.push("    [--] no tasks under this mission");
      }
    }
  }

  lines.push("", "> Next ready");
  if (report.next_ready) {
    const t = report.next_ready;
    lines.push(`  [ok] ${t.id} ${t.slug} -- ${t.title}`);
  } else {
    lines.push("  [--] no tasks ready to ship");
  }

  if (!terse) {
    lines.push("", "> Recent transitions");
    if (report.recent_transitions.length === 0) {
      lines.push("  [--] no transitions yet");
    } else {
      for (const row of report.recent_transitions) {
        const subjectId = row.task_id ?? row.mission_id ?? "-";
        const outcome = row.verdict ?? row.to_state;
        lines.push(`  ${row.timestamp}  ${subjectId}  ${row.trigger_verb}  ${outcome}`);
      }
    }
  }

  return lines;
}

function healthEntries(
  health: SetupCheckReport | readonly SetupCheckEntry[],
): readonly SetupCheckEntry[] {
  return Array.isArray(health) ? health : (health as SetupCheckReport).entries;
}

function glyph(status: SetupCheckEntry["status"]): string {
  if (status === "ok") return "[ok]";
  if (status === "warn") return "[--]";
  return "[!]";
}

function formatMissionHeader(group: MissionGroup): string {
  const m = group.mission;
  if ("synthetic" in m) {
    return `(unscoped)  ${group.tasks.length} task(s)`;
  }
  return `${m.id}  ${m.status}  ${m.title}`.trim();
}

function formatTaskLine(
  id: string,
  slug: string,
  state: string,
  signal: TaskSignal,
): string {
  const base = `${id}  ${state.padEnd(9)}  ${slug}`;
  if (signal.kind === "verdict") return `${base}  [verdict: ${signal.decision}]`;
  if (signal.kind === "transition") {
    return `${base}  [last: ${signal.trigger_verb} -> ${signal.to_state}]`;
  }
  return base;
}
