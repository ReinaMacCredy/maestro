import type { SetupCheckReport, SetupCheckEntry } from "@/service/setup-check.usecase.js";
import type { LatestVerdictSummary } from "@/service/load-latest-verdicts.usecase.js";
import type { TransitionEvidenceRow } from "@/repo/evidence-store.port.js";
import type { Task } from "@/types/task.js";
import type { Mission } from "@/shared/domain/legacy-mission";
import type { VerdictDecision } from "@/features/verdict/domain/types.js";

export type { LatestVerdictSummary };

// DoctorCheck is structurally compatible with SetupCheckEntry but uses
// "fail" instead of "missing" for the status field. The TUI uses DoctorCheck
// for its own checks (git, config) while SetupCheckReport uses SetupCheckEntry
// for scaffold checks. Both are spread into the same checks array in the TUI.
export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

// Re-export SetupCheckEntry for use in status report
export type { SetupCheckEntry };

export interface EnvironmentStatus {
  readonly initialized: boolean;
  readonly configSource: "global" | "project" | "none";
  readonly gitAvailable: boolean;
}

export interface ProjectVerifiedState {
  readonly latest_verdict: LatestVerdictSummary | undefined;
  readonly stuck_verifying_count: number;
  readonly stale_handoff_count: number;
  readonly corrupt_verdict_count: number;
}

export type TaskSignal =
  | {
      readonly kind: "verdict";
      readonly decision: VerdictDecision;
      readonly computedAt: string;
    }
  | {
      readonly kind: "transition";
      readonly to_state: string;
      readonly trigger_verb: string;
      readonly timestamp: string;
    }
  | { readonly kind: "none" };

export interface UnscopedMissionMarker {
  readonly id: "(unscoped)";
  readonly title: string;
  readonly synthetic: true;
}

export interface TaskWithSignal {
  readonly task: Task;
  readonly signal: TaskSignal;
}

export interface MissionGroup {
  readonly mission: Mission | UnscopedMissionMarker;
  readonly tasks: readonly TaskWithSignal[];
}

export interface StatusReport {
  readonly maestro_health: SetupCheckReport;
  readonly project_state: ProjectVerifiedState;
  readonly missions: readonly MissionGroup[];
  readonly next_ready: Task | undefined;
  readonly recent_transitions: readonly TransitionEvidenceRow[];
}
