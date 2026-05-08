import type { Services } from "@/services.js";

export interface RegisterDeps {
  readonly getServices: () => Services;
  readonly sessionId: string;
}
