import type { Correction, CreateCorrectionInput } from "../domain/memory-types.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";

export interface CaptureOpts {
  readonly rule: string;
  readonly source: string;
  readonly keywords: readonly string[];
  readonly fileGlobs: readonly string[];
  readonly severity: "soft" | "hard";
}

export async function captureCorrection(
  store: CorrectionStorePort,
  opts: CaptureOpts,
): Promise<Correction> {
  const input: CreateCorrectionInput = {
    rule: opts.rule,
    source: opts.source,
    trigger: {
      keywords: opts.keywords,
      fileGlobs: opts.fileGlobs,
    },
    severity: opts.severity,
  };
  return store.create(input);
}
