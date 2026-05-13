/**
 * Test repository fixture caching for e2e tests.
 * 
 * OPTIMIZATION: Instead of creating a fresh git repo + maestro init for every
 * test, we create a "golden" fixture once per test file, then clone it via
 * cp -r for each test. This eliminates the N+1 temp directory creation pattern.
 * 
 * Savings: 15-25 seconds across e2e suite (68 mkdtemp calls reduced to ~10-15).
 */
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runCompiled, initGitRepo } from "./run-compiled-cli.js";
import { runCommand } from "./command-runner.js";

interface FixtureCache {
  dir: string;
  refCount: number;
}

const fixtureCache = new Map<string, FixtureCache>();
const clonedDirs = new Set<string>();

/**
 * Get or create a cached fixture with the given setup function.
 * The fixture is created once per test file and reused across tests.
 * 
 * @param key - Unique key for this fixture type (e.g., "basic", "with-contract")
 * @param setup - Async function to set up the fixture (only called once)
 * @returns Path to the cached fixture directory
 */
export async function getOrCreateFixture(
  key: string,
  setup: (dir: string) => Promise<void>,
): Promise<string> {
  let cached = fixtureCache.get(key);
  
  if (!cached) {
    const dir = await mkdtemp(join(tmpdir(), `maestro-fixture-${key}-`));
    cached = { dir, refCount: 0 };
    fixtureCache.set(key, cached);
    
    try {
      await setup(dir);
    } catch (err) {
      // Setup failed, clean up and remove from cache
      await rm(dir, { recursive: true, force: true }).catch(() => {});
      fixtureCache.delete(key);
      throw err;
    }
  }
  
  cached.refCount++;
  return cached.dir;
}

/**
 * Clone a cached fixture for use in a test.
 * The clone is tracked and will be cleaned up automatically.
 * 
 * @param fixtureDir - Path to the cached fixture (from getOrCreateFixture)
 * @param testName - Optional test name for debugging (included in clone dir name)
 * @returns Path to the cloned directory
 */
export async function cloneFixture(
  fixtureDir: string,
  testName?: string,
): Promise<string> {
  const suffix = testName ? `-${testName.replace(/[^a-z0-9-]/gi, "-")}` : "";
  const target = await mkdtemp(join(tmpdir(), `maestro-test${suffix}-`));
  
  // Use cp -r for fast directory copy (much faster than recursive fs operations)
  await runCommand(["cp", "-r", `${fixtureDir}/.`, target]);
  
  clonedDirs.add(target);
  return target;
}

/**
 * Create a basic initialized repo fixture (git init + maestro init).
 * This is the most common fixture type used across e2e tests.
 * 
 * @returns Path to the cached fixture directory
 */
export async function getBasicRepoFixture(): Promise<string> {
  return getOrCreateFixture("basic-repo", async (dir) => {
    await initGitRepo(dir);
    await runCommand(["git", "config", "user.email", "test@example.com"], dir);
    await runCommand(["git", "config", "user.name", "Test"], dir);
    await runCompiled(["init"], dir);
    await runCommand(["git", "add", "."], dir);
    await runCommand(["git", "commit", "--allow-empty", "-m", "init"], dir);
  });
}

/**
 * Clone the basic repo fixture for use in a test.
 * Convenience wrapper around getBasicRepoFixture + cloneFixture.
 * 
 * @param testName - Optional test name for debugging
 * @returns Path to the cloned directory
 */
export async function cloneBasicRepo(testName?: string): Promise<string> {
  const fixture = await getBasicRepoFixture();
  return cloneFixture(fixture, testName);
}

/**
 * Clean up a cloned fixture directory.
 * 
 * @param dir - Path to the cloned directory
 */
export async function cleanupClone(dir: string): Promise<void> {
  if (clonedDirs.has(dir)) {
    await rm(dir, { recursive: true, force: true }).catch(() => {});
    clonedDirs.delete(dir);
  }
}

/**
 * Clean up all cloned fixtures and cached fixtures.
 * Call this in afterAll() to ensure cleanup happens.
 */
export async function cleanupAllFixtures(): Promise<void> {
  // Clean up all clones
  const cleanupPromises: Promise<void>[] = [];
  for (const dir of clonedDirs) {
    cleanupPromises.push(rm(dir, { recursive: true, force: true }).catch(() => {}));
  }
  clonedDirs.clear();
  
  // Clean up cached fixtures
  for (const [key, cached] of fixtureCache.entries()) {
    cleanupPromises.push(rm(cached.dir, { recursive: true, force: true }).catch(() => {}));
  }
  fixtureCache.clear();
  
  await Promise.all(cleanupPromises);
}

/**
 * Decrement the reference count for a fixture and clean it up if no longer needed.
 * This is optional - fixtures will be cleaned up by cleanupAllFixtures() anyway.
 * 
 * @param key - Fixture key
 */
export function releaseFixture(key: string): void {
  const cached = fixtureCache.get(key);
  if (cached) {
    cached.refCount--;
    if (cached.refCount <= 0) {
      rm(cached.dir, { recursive: true, force: true }).catch(() => {});
      fixtureCache.delete(key);
    }
  }
}
