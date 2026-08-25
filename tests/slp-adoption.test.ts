import { expect, test } from "bun:test";
import { join } from "node:path";
import { prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

test("133 install materializes the dispatch, handback, dependency, and episode contracts", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skill = await Bun.file(
      join(fixture.home, ".agents", "skills", "maestro-work", "SKILL.md"),
    ).text();
    for (const required of [
      "## Dispatch",
      "## Handback",
      "Lane",
      "Excluded scope",
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
      "A+B+C",
      "Attempted",
      "Invariant assumed",
      "Exact failure",
      "What changed between attempts",
      "What did not change",
      "Smallest new information needed",
    ]) {
      expect(skill).toContain(required);
    }
  });
});

test("134 install materializes layered proof, failed traces, learning, and triage", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const verifyRoot = join(fixture.home, ".agents", "skills", "maestro-verify");
    const skill = await Bun.file(join(verifyRoot, "SKILL.md")).text();
    for (const required of [
      "source",
      "artifact",
      "installed",
      "live",
      "journey",
      "NOT TESTED",
      "Assumptions not verified",
      "Residual risks",
      '"failed: ',
    ]) {
      expect(skill).toContain(required);
    }

    const learning = await Bun.file(join(verifyRoot, "references", "learning.md")).text();
    expect(learning).toContain("canary");
    expect(learning).toContain("review/delete date");

    const triagePath = join(verifyRoot, "references", "triage.md");
    expect(await Bun.file(triagePath).exists()).toBe(true);
    const triage = await Bun.file(triagePath).text();
    for (const step of [
      "Problem",
      "Authority",
      "Topology",
      "Attention",
      "Capability",
      "State",
      "Evidence",
      "Owning layer",
      "Learning",
    ]) {
      expect(triage).toContain(step);
    }

    const audit = await Bun.file(join(verifyRoot, "references", "audit.md")).text();
    expect(audit).toContain("triage.md");
  });
});

test("135 install materializes intake, council reconcile, and handoff doctrine", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const skillsRoot = join(fixture.home, ".agents", "skills");
    const design = await Bun.file(join(skillsRoot, "maestro-design", "SKILL.md")).text();
    expect(design.indexOf("## Intake")).toBeGreaterThan(-1);
    expect(design.indexOf("## Intake")).toBeLessThan(design.indexOf("## Recall pass first"));
    for (const required of [
      "state unknown",
      "several architectures",
      "contract clear",
      "candidate needs breaking",
      "hard-to-reverse fork",
      "ROI",
      "## Council",
      "premise",
      "mechanism",
      "boundary",
      "failure",
      "reversibility",
      "evidence",
      "authority",
      "proof",
    ]) {
      expect(design).toContain(required);
    }

    const bundle = await Bun.file(join(skillsRoot, "maestro-bundle", "SKILL.md")).text();
    for (const required of [
      "break-before-make",
      "owner changes",
      "dependency becomes its own branch",
      "role changes",
      "context is full of false starts",
    ]) {
      expect(bundle).toContain(required);
    }
  });
});

test("136 bundle open scaffolds the NOTES handoff packet after Next Action", async () => {
  await withFixture(async (fixture) => {
    const opened = await runCli(fixture, ["bundle", "open", "slp-handoff"]);
    expect(opened.exitCode).toBe(0);

    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "slp-handoff", "NOTES.md"),
    ).text();
    const nextAction = notes.indexOf("## Next Action");
    const authority = notes.indexOf("## Authority");
    const failed = notes.indexOf("## Failed approaches");
    const doNotRepeat = notes.indexOf("## Do not repeat");
    expect(authority).toBeGreaterThan(nextAction);
    expect(failed).toBeGreaterThan(authority);
    expect(doNotRepeat).toBeGreaterThan(failed);
  });
});

test("137 SessionStart adds only the intake line and UserPromptSubmit stays byte-identical", async () => {
  await withFixture(async (fixture) => {
    const session = {
      MAESTRO_SESSION_ID: "slp-brief",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(start.exitCode).toBe(0);
    expect(start.stdout).toContain(
      '  close: maestro bundle close <id> after VERIFY passes; recall with maestro search "<term>"\n' +
        "intake: problem in one sentence; uncertainty -> lane (scout no-write | decision x2-3 | delivery | challenge); ROI 0-10 -> tier\n",
    );

    const prompt = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session,
    );
    expect(prompt.exitCode).toBe(0);
    expect(prompt.stdout).toBe(
      "held work: none\n" +
        "enabled policies: policy-breakdown, policy-lifecycle, policy-proof\n" +
        "0 pending messages\n" +
        "next: maestro ready\n" +
        "recipes: maestro recipe list; maestro recipe show <name>\n",
    );
  });
});
