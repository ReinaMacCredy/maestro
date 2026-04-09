export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

/**
 * Summary record returned by `checkStatus` for the CLI `status` command
 * and Mission Control environment panels. The `pendingHandoffs` list
 * is a narrow projection of the full `UkiHandoff` records -- just what
 * status/CLI surfaces need to count and label pending work. Richer
 * consumers (TUI handoff modal) read full records from the handoff
 * store directly via its port.
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
