export interface GitSignatureProbePort {
  /**
   * Returns the raw output of `git log --show-signature <base>..<head>`.
   * Callers parse the output to detect unsigned commits.
   */
  showSignatureLog(input: {
    readonly repoRoot: string;
    readonly base: string;
    readonly head: string;
  }): Promise<string>;
}
