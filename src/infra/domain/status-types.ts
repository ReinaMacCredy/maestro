export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

/**
 * Status summary returned by the CLI `status` command.
 *
 * `pendingHandoffs` remains a narrow summary projection for stable JSON
 * consumers; richer handoff views read full records from the handoff store.
 */
export interface StatusReport {
  readonly initialized: boolean;
  readonly configSource: "global" | "project" | "none";
  readonly pendingHandoffs: readonly PendingHandoffSummary[];
  /**
   * Phase 1 strip: cassAvailable is still on the struct for the same
   * structural reason as pendingHandoffs. Value is always false until
   * the field is removed outright in a later phase.
   */
  readonly cassAvailable: boolean;
  readonly gitAvailable: boolean;
}

export interface PendingHandoffSummary {
  readonly id: string;
  readonly agent: string;
  readonly createdAt: string;
}
