import type { CassSearchResponse } from "../domain/types.js";

export interface CassPort {
  isAvailable(): Promise<boolean>;
  hasBinary(): Promise<boolean>;
  indexOnce(sessionPaths: readonly string[]): Promise<void>;
  search(
    query: string,
    options: {
      agent?: string;
      workspace?: string;
      limit?: number;
    },
  ): Promise<CassSearchResponse>;
}
