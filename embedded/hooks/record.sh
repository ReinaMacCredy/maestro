#!/bin/sh
# maestro:hook-version: 1.0.2
#
# Maestro hook recorder entry point.
#
# Maestro installs this script as the `command` for every hook event in
# .claude/settings.local.json and .codex/hooks.json. The agent runs it with the
# event payload on stdin; the script forwards to the Maestro binary that
# installed it, which reads the payload, keeps only accepted events, and appends
# evidence to .maestro/runs/<session>/events.jsonl (writing run_evidence.yaml on
# Stop).
#
# This file is yours to edit: wrap or extend recording as you like. It survives
# `maestro upgrade` until the shipped `maestro:hook-version` above changes, at
# which point your copy is backed up and the bundled version restored. Delete it
# and re-run `maestro init` to recover the bundled script.
MAESTRO_BIN=@MAESTRO_BIN@
exec "$MAESTRO_BIN" hook record
