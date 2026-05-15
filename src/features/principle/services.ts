import type { PrincipleStorePort } from "./ports/principle-store.port.js";
import { JsonlPrincipleStoreAdapter } from "./adapters/jsonl-principle-store.adapter.js";

export interface PrincipleServices {
  readonly principleStore: PrincipleStorePort;
}

export function buildPrincipleServices(projectDir: string): PrincipleServices {
  const principleStore = new JsonlPrincipleStoreAdapter(projectDir);
  return { principleStore };
}
