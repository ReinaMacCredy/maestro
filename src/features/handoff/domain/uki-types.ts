/**
 * UKI handoff domain types.
 *
 * The persisted handoff source of truth is now a structured JSON payload
 * (`content`) with two explicit modes: `plan` and `execute`. The raw UKI
 * string remains a cached transfer rendering for direct agent handoff.
 */

export const UKI_HANDOFF_STATUSES = ["pending", "picked-up", "completed"] as const;
export const UKI_HANDOFF_MODES = ["plan", "execute"] as const;

export type UkiHandoffStatus = (typeof UKI_HANDOFF_STATUSES)[number];
export type UkiHandoffMode = (typeof UKI_HANDOFF_MODES)[number];

export const UKI_HANDOFF_VERSION = "5.4";
export const SUPPORTED_UKI_HANDOFF_VERSIONS = ["5.2", "5.3", UKI_HANDOFF_VERSION] as const;

export type UkiHandoffVersion = (typeof SUPPORTED_UKI_HANDOFF_VERSIONS)[number];

export interface UkiConfidenceScores {
  readonly work?: number;
  readonly summary?: number;
}

export interface UkiMaestroRefs {
  readonly missionId?: string;
  readonly featureId?: string;
  readonly milestoneId?: string;
  readonly planPath?: string;
  readonly specPath?: string;
}

export interface UkiVerificationResult {
  readonly step: string;
  readonly passed: boolean;
}

export interface UkiHandoffContentBase {
  readonly mode: UkiHandoffMode;
  readonly currentState: string;
  readonly sessionCore: string;
  readonly decisions: readonly string[];
  readonly artifacts: readonly string[];
  readonly readMore: readonly string[];
  readonly nextAction: string;
  readonly summary: string;
  readonly maestroRefs: UkiMaestroRefs;
  readonly cs: UkiConfidenceScores;
  readonly signalDelta: readonly string[];
  readonly boundaryState: readonly string[];
  readonly risks: readonly string[];
  readonly blindSpot?: string;
  readonly metaphor?: string;
  readonly causalDrivers: readonly string[];
  readonly divergences: readonly string[];
  readonly assumptions?: readonly string[];
  readonly scopeDeclaration?: Readonly<Record<string, string>>;
  readonly complexityDelta?: Readonly<Record<string, unknown>>;
  readonly verificationResults?: readonly UkiVerificationResult[];
}

export interface PlanUkiHandoffContent extends UkiHandoffContentBase {
  readonly mode: "plan";
  readonly planPaths: readonly string[];
  readonly maestroSync: readonly string[];
}

export interface ExecuteUkiHandoffContent extends UkiHandoffContentBase {
  readonly mode: "execute";
  readonly touchedFiles: readonly string[];
  readonly completedWork: readonly string[];
  readonly validation: readonly string[];
}

export type UkiHandoffContent = PlanUkiHandoffContent | ExecuteUkiHandoffContent;

export interface UkiHandoff {
  readonly id: string;
  readonly version: UkiHandoffVersion;
  readonly timestamp: string;
  readonly status: UkiHandoffStatus;
  readonly agent: string;
  readonly sessionId: string;
  readonly content: UkiHandoffContent;
  readonly uki: string;
  readonly pickedUpAt?: string;
  readonly pickedUpBy?: string;
  readonly completedAt?: string;
  readonly report?: string;
}

export interface CreateUkiHandoffInput {
  readonly content: UkiHandoffContent;
  readonly agent: string;
  readonly sessionId: string;
}
