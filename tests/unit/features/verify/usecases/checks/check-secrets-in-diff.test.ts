import { describe, expect, it } from "bun:test";
import { checkSecretsInDiff } from "@/features/verify/usecases/checks/check-secrets-in-diff.js";

describe("checkSecretsInDiff", () => {
  it("clean diff — empty findings", () => {
    const findings = checkSecretsInDiff([
      "+const x = 1;",
      "+function hello() { return 'hi'; }",
    ]);
    expect(findings).toEqual([]);
  });

  it("AWS access key id — emits error finding", () => {
    const findings = checkSecretsInDiff([
      "+const AWS_KEY = 'AKIAIOSFODNN7EXAMPLE';",
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.check).toBe("secrets-in-diff");
    expect(findings[0]?.severity).toBe("error");
    expect(findings[0]?.details).toMatch(/aws-access-key-id/);
  });

  it("GitHub PAT — emits error finding", () => {
    const findings = checkSecretsInDiff([
      "+const token = 'ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZaBcDeFgHiJ';",
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.details).toMatch(/github-pat/);
  });

  it("Slack token — emits error finding", () => {
    const findings = checkSecretsInDiff([
      "+const slack = 'xoxb-abcdefghij-1234567890';",
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.details).toMatch(/slack-token/);
  });

  it("PEM private key block — emits error finding", () => {
    const findings = checkSecretsInDiff([
      "+-----BEGIN RSA PRIVATE KEY-----",
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.details).toMatch(/pem-private-key/);
  });

  it("high-entropy string near 'secret' keyword — emits error finding", () => {
    const findings = checkSecretsInDiff([
      "+const secret = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrst';",
    ]);
    expect(findings.some((f) => f.check === "secrets-in-diff" && f.severity === "error")).toBe(true);
  });

  it("benign long string without credential keyword — no finding", () => {
    // A long description text without key/token/secret/password context
    const findings = checkSecretsInDiff([
      "+const description = 'This is a really long string that has absolutely nothing to do with credentials or sensitive data at all in any way';",
    ]);
    const secretFindings = findings.filter((f) => f.details?.includes("high-entropy"));
    expect(secretFindings).toHaveLength(0);
  });

  it("multiple violations — each gets its own finding", () => {
    const findings = checkSecretsInDiff([
      "+const awsKey = 'AKIAIOSFODNN7EXAMPLE';",
      "+const ghToken = 'ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZaBcDeFgHiJ';",
    ]);
    const checks = new Set(findings.map((f) => f.details));
    expect(checks.size).toBeGreaterThanOrEqual(2);
  });

  it("empty diff — empty findings", () => {
    expect(checkSecretsInDiff([])).toEqual([]);
  });
});
