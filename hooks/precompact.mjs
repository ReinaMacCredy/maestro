import {
  findMaestroProjectRoot,
  formatPrecompactContext,
  resolveLatestTaskContinuation,
} from "./_task-continuation.mjs";

async function main() {
  const projectRoot = await findMaestroProjectRoot();
  if (!projectRoot) {
    return;
  }

  const result = await resolveLatestTaskContinuation(projectRoot);
  const context = formatPrecompactContext(result);
  if (!context) {
    return;
  }

  process.stdout.write(`${context}\n`);
}

main().catch(() => {
  process.exit(0);
});
