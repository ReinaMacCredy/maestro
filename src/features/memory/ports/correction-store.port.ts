import type { Correction, CreateCorrectionInput, CorrectionQuery } from "../domain/memory-types.js";

export interface CorrectionStorePort {
  create(input: CreateCorrectionInput): Promise<Correction>;
  get(id: string): Promise<Correction | undefined>;
  list(): Promise<readonly Correction[]>;
  search(query: CorrectionQuery): Promise<readonly Correction[]>;
  update(id: string, input: Partial<Correction>): Promise<Correction | undefined>;
  remove(id: string): Promise<boolean>;
}
