import { homedir } from "node:os";
import { join } from "node:path";

export const MAESTRO_DIR = ".maestro";

export const MEMORY_DIR = "memory";

export const GRAPH_DIR = join(homedir(), ".maestro", "graph");
