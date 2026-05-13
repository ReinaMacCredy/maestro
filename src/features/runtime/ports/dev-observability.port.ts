/**
 * Dev-time per-worktree observability for the agent: query metrics and tail
 * logs as the work happens, not as a deploy gate.
 *
 * Distinct from {@link RuntimeMonitorPort}: that one is L7 release-gate
 * observability driven by `Spec.runtime_signals` and consumed by
 * `maestro runtime check`. This port is consumed by `maestro task observe`
 * and produces no Verdict-bearing Evidence by default (`--record` opts in to
 * a `manual-note`-shaped `dev-observation` row).
 *
 * See `docs/dev-observability.md` for the principle-to-primitive mapping
 * and recipe examples.
 */

export interface DevMetricSample {
  /** The scalar value parsed from the metric backend. */
  readonly value: number;
  /** ISO 8601 timestamp the query was issued at. */
  readonly sampledAt: string;
  /** Human-readable source label so multi-adapter setups can be disambiguated. */
  readonly source: string;
}

export interface DevLogLine {
  readonly text: string;
  /** Best-effort byte offset from the file head for cursor-resume callers. */
  readonly offset?: number;
}

export interface DevLogTail {
  readonly lines: readonly DevLogLine[];
  readonly source: string;
}

export interface DevObservabilityPort {
  /**
   * Issue a metric query against a metrics backend. The implementation
   * decides the query language; the port stays language-agnostic so a Loki
   * or OTLP adapter can land later without breaking the command surface.
   */
  queryMetric(query: string): Promise<DevMetricSample>;

  /**
   * Tail the most recent `lines` (default-bound by the adapter) from a log
   * source matching `filter`. Filter semantics are adapter-defined; the
   * file adapter treats it as a substring/regex match.
   */
  tailLogs(filter: string | undefined, lines?: number): Promise<DevLogTail>;
}
