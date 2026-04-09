import type { RatchetSuite, RatchetBaseline } from "../domain/memory-types.js";

export interface RatchetStorePort {
  getSuite(): Promise<RatchetSuite>;
  writeSuite(suite: RatchetSuite): Promise<void>;
  getBaseline(): Promise<RatchetBaseline | undefined>;
  writeBaseline(baseline: RatchetBaseline): Promise<void>;
}
