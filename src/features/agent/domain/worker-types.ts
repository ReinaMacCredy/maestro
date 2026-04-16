export interface CliWorkerConfig {
  readonly enabled: boolean;
  readonly transport: "cli";
  readonly command: string;
  readonly args?: readonly string[];
  readonly env?: Readonly<Record<string, string>>;
}

export type WorkerConfig = CliWorkerConfig;
