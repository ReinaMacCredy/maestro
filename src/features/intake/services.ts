// Intake is a pure-compute feature: no ports, no adapters, no persistence.
// The Services interface keeps a placeholder for shape consistency with other
// features.
export interface IntakeServices { /* Reserved */ }

export function buildIntakeServices(): IntakeServices {
  return {};
}
