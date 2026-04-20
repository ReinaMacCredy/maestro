export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

/**
 * Status summary returned by the CLI `status` command.
 */
export interface StatusReport {
  readonly initialized: boolean;
  readonly configSource: "global" | "project" | "none";
  readonly gitAvailable: boolean;
  readonly legacyHandoffCount: number;
}
