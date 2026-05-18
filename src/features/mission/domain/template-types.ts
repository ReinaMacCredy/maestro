export interface MissionTemplateSeedTask {
  readonly title: string;
  readonly slug: string;
}

export interface MissionTemplate {
  readonly name: string;
  readonly description: string;
  readonly seedTasks: readonly MissionTemplateSeedTask[];
  readonly source: "builtin" | "user";
}

export class MissionTemplateLoadError extends Error {
  readonly filePath: string;
  readonly issuePath?: string;
  constructor(filePath: string, issue: string, issuePath?: string) {
    const prefix = issuePath ? `${filePath}: ${issuePath}` : filePath;
    super(`${prefix}: ${issue}`);
    this.name = "MissionTemplateLoadError";
    this.filePath = filePath;
    this.issuePath = issuePath;
  }
}
