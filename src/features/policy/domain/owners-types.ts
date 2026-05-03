/** Domain shape: camelCase, used internally. */
export interface Owners {
  readonly policyApprovers: readonly string[];
  readonly ratchetApprovers: readonly string[];
  readonly sensitiveWaivers: readonly string[];
}

/** Raw YAML shape: snake_case keys as written in owners.yaml. */
export interface OwnersYaml {
  readonly policy_approver?: readonly string[] | null;
  readonly ratchet_approver?: readonly string[] | null;
  readonly sensitive_waiver?: readonly string[] | null;
}
