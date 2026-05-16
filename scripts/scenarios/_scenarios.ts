// Shared constant: all 8 Phase 6 scenario names + type derivation.
// Import this from check.ts, check-all.ts, and swarm.ts.

export const SCENARIO_NAMES = [
  "greenfield-novice-light",
  "greenfield-novice-heavy",
  "greenfield-expert-light",
  "greenfield-expert-heavy",
  "brownfield-novice-light",
  "brownfield-novice-heavy",
  "brownfield-expert-light",
  "brownfield-expert-heavy",
] as const;

export type ScenarioName = (typeof SCENARIO_NAMES)[number];

export function projectTypeOf(name: string): "greenfield" | "brownfield" {
  return name.startsWith("brownfield") ? "brownfield" : "greenfield";
}

export function isKnownScenario(name: string): name is ScenarioName {
  return (SCENARIO_NAMES as readonly string[]).includes(name);
}
