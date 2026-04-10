import type { UkiHandoff } from "@/features/handoff";

export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

/**
 * Status summary returned by the CLI `status` command.
 *
 * `pendingHandoffs` preserves the full persisted record shape for
 * backward-compatible JSON consumers.
 */
export interface StatusReport {
  readonly initialized: boolean;
  readonly configSource: "global" | "project" | "none";
  readonly pendingHandoffs: readonly UkiHandoff[];
  /**
   * Phase 1 strip: cassAvailable is still on the struct for the same
   * structural reason as pendingHandoffs. Value is always false until
   * the field is removed outright in a later phase.
   */
  readonly cassAvailable: boolean;
  readonly gitAvailable: boolean;
}
