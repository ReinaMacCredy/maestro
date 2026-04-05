const DEFAULT_CAPTURE_MAX_CHARS = 128_000;
const DEFAULT_CAPTURE_HEAD_CHARS = 32_000;

export interface OutputCaptureOptions {
  readonly maxChars?: number;
  readonly headChars?: number;
}

export class OutputCapture {
  private readonly maxChars: number;
  private readonly headChars: number;
  private readonly tailChars: number;
  private readonly headLines: string[] = [];
  private readonly tailLines: string[] = [];
  private headLength = 0;
  private tailLength = 0;
  private totalLength = 0;
  private firstNonEmptyLineValue: string | undefined;

  constructor(options: OutputCaptureOptions = {}) {
    const maxChars = Math.max(1, options.maxChars ?? DEFAULT_CAPTURE_MAX_CHARS);
    const requestedHeadChars = options.headChars ?? Math.min(DEFAULT_CAPTURE_HEAD_CHARS, maxChars);
    this.maxChars = maxChars;
    this.headChars = Math.min(maxChars, Math.max(0, requestedHeadChars));
    this.tailChars = Math.max(0, maxChars - this.headChars);
  }

  appendLine(line: string): void {
    const normalized = line.replace(/\r/g, "");
    if (normalized.length > 0 && this.firstNonEmptyLineValue === undefined) {
      this.firstNonEmptyLineValue = normalized;
    }

    this.totalLength += normalized.length;

    if (this.headLength < this.headChars) {
      this.headLines.push(normalized);
      this.headLength += normalized.length;
      return;
    }

    if (this.tailChars === 0) {
      return;
    }

    this.tailLines.push(normalized);
    this.tailLength += normalized.length;
    while (this.tailLength > this.tailChars && this.tailLines.length > 0) {
      const removed = this.tailLines.shift() ?? "";
      this.tailLength -= removed.length;
    }
  }

  appendTextBlock(text: string): void {
    const normalized = text.replace(/\r/g, "");
    if (normalized.length === 0) {
      return;
    }

    const lines = normalized.split("\n");
    for (const line of lines) {
      this.appendLine(line);
    }
  }

  get firstNonEmptyLine(): string | undefined {
    return this.firstNonEmptyLineValue;
  }

  get isTruncated(): boolean {
    return this.totalLength > this.headLength + this.tailLength;
  }

  toString(): string {
    const head = this.headLines.join("\n").trim();
    if (!this.isTruncated) {
      return head;
    }

    const omittedChars = Math.max(0, this.totalLength - this.headLength - this.tailLength);
    const tail = this.tailLines.join("\n").trim();
    const marker = `...[truncated ${omittedChars} chars]...`;
    return [head, marker, tail]
      .filter((part) => part.length > 0)
      .join("\n")
      .trim();
  }
}

export function createOutputCapture(options?: OutputCaptureOptions): OutputCapture {
  return new OutputCapture(options);
}

