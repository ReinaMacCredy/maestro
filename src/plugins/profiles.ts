// v0.126.0's `maestro update` imports this path after the runtime swap and calls materializeProfiles; without it the first update to this version fails.
import { syncSeatFiles } from "./install.ts";

export async function materializeProfiles(home: string, repo: string): Promise<void> {
  await syncSeatFiles(home, repo);
}
