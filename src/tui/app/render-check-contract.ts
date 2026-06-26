export interface RenderCheckScreenResult {
  readonly screen: string;
  readonly status: "pass" | "fail" | "skip";
  readonly size: string;
  readonly warnings: string[];
}

export interface RenderCheckResult {
  readonly screens: RenderCheckScreenResult[];
  readonly summary: {
    readonly total: number;
    readonly passed: number;
    readonly failed: number;
    readonly skipped: number;
  };
}
