export interface GitFileChange {
  readonly path: string;
  readonly kind: "added" | "modified" | "deleted";
}

export interface GitState {
  readonly branch: string;
  readonly dirty: boolean;
}

export interface GitWorktree {
  readonly root: string;
  readonly branch: string;
}
