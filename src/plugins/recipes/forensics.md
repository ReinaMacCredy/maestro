# Forensics

Use this recipe to diagnose a runtime symptom (a leak, an idle CPU spin, a
visual glitch) from evidence the process produced. It has two branches. The
live-process branch instruments a process that still runs. The
captured-artifact branch reads a capture handed to you after the fact: a
cpuprofile, a trace, a spindump, a heap snapshot. You own the diagnosis.
Measure, do not theorize from source. The deliverable is a cited diagnosis, not
a fix. When the owner also asked for the fix, hand the known cause to
`maestro recipe show bug-fix`, or to `maestro recipe show perf-issue` for a
slowness. Ported from pstack's runtime-forensics and trace-forensics playbooks
(Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Get the artifact, on the branch that fits.
   - Live process. Capture the live signal through the repository's
     verify-<repo> skill (`maestro recipe show verification-skill`): a CPU
     profile for a spinning process, a heap snapshot for a leak, a CDP trace
     for a visual glitch. A real artifact, not a guess.
   - Captured artifact. The capture is a fixed dataset. Read it, do not re-run
     it. Identify the format and load it with the right tool: a DevTools or
     trace parser for cpuprofile and `.json.gz`, a text editor for a
     spindump, your heap tooling for a heap snapshot.
   - Either way, save the artifact in the topic folder
     (`maestro topic path <slug>`).
2. Reach a queryable shape before you read. Dump the trace or heap snapshot
   into sqlite, one row per sample, frame or node. Parse a large artifact in a
   subagent under the `delegate` profile and keep only the reduced finding in
   the main thread, per **Guard the Context Window** (PRINCIPLES.md).
3. Narrow to the cause. Query for the frames that hold the most time and walk
   the call tree to the hot path. For a leak, follow the retainer chain from
   the leaked object to a GC root. For a spin, find the loop that fires
   without input. For a spindump, find the thread stuck on CPU or blocked, and
   its wait reason.
4. Confirm the mechanism before you believe it.
   - Live process. Inject instrumentation into the running process, through a
     CDP eval or a hotfix of the live code without a reload, to confirm the
     hypothesis cheaply. Remove the instrumentation afterwards. The process
     and the source stay as you found them.
   - Captured artifact. With a paired capture, diff the before and after
     artifacts to confirm. Without one, mark the finding as the strongest
     hypothesis the artifact supports, not a confirmed cause.
5. Attribute the finding to source. Name the file, the symbol, and the line
   that allocates or schedules, through the artifact's own symbols. A frame
   with no source mapping is not a diagnosis yet. Resolve the symbols, or say
   plainly that the artifact does not carry them.
6. Record the diagnosis. With a work item, run
   `maestro work note <id> "cause: <one line>; source: <file:line>; artifacts: <paths>"`.
   Without one, the record goes in the reply. The record holds the cause, its
   evidence chain, the source location and the artifact paths.
7. Keep the throughput checkpoint to one plan-list item,
   `throughput checkpoint: n/a, read-only forensics`.
8. Hand back the cited diagnosis. No fix unless asked. Once the cause is
   known, a fix request goes to `maestro recipe show bug-fix` or
   `maestro recipe show perf-issue`.

## Reply

The branch taken. The signal captured, or the artifact and its format. The
reduced finding. How you proved the mechanism, or whether a paired capture
confirmed it. The source location. The artifact paths.
