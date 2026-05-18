// Shared constant: the scenario names.
// Import this from check.ts, check-all.ts, and swarm.ts.

export const SCENARIO_NAMES = [
  "greenfield-novice-light",
  "greenfield-novice-heavy",
  "greenfield-expert-light",
  "greenfield-expert-heavy",
] as const;

export type ScenarioName = (typeof SCENARIO_NAMES)[number];

export function isKnownScenario(name: string): name is ScenarioName {
  return (SCENARIO_NAMES as readonly string[]).includes(name);
}
