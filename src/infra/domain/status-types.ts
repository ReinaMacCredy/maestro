export interface DoctorCheck {
  readonly id?: string;
  readonly name?: string;
  readonly label?: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly detail?: string;
  readonly fix?: string;
}

export interface EnvironmentStatus {
  readonly checks: readonly DoctorCheck[];
}
