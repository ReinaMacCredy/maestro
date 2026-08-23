#!/usr/bin/env bun

import { run } from "../src/kernel/index.ts";

process.exitCode = await run(process.argv.slice(2));
