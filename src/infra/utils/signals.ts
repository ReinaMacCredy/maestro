type CleanupFn = () => Promise<void> | void;

const cleanupRegistry: Map<string, CleanupFn> = new Map();

export function registerCleanup(id: string, fn: CleanupFn): void {
  cleanupRegistry.set(id, fn);
}

export function unregisterCleanup(id: string): void {
  cleanupRegistry.delete(id);
}

/** @internal test-only */
export function _registrySize(): number {
  return cleanupRegistry.size;
}

async function runCleanup(): Promise<void> {
  for (const [id, fn] of cleanupRegistry) {
    try {
      await fn();
    } catch (err) {
      console.error(`[warn] cleanup failed for ${id}:`, err);
    }
  }
  cleanupRegistry.clear();
}

let installed = false;

export function installSignalHandlers(): void {
  if (installed) return;
  installed = true;

  const handler = async (signal: string) => {
    console.error(`\n[${signal}] shutting down...`);
    await runCleanup();
    process.exit(signal === 'SIGINT' ? 130 : 143);
  };

  process.on('SIGINT', () => handler('SIGINT'));
  process.on('SIGTERM', () => handler('SIGTERM'));
}
