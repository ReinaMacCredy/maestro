# V4 final cumulative chain

This Orchestrator-owned namespace implements the external proof-control contract
locked by `dec-canonical-final-cumulative-stage-0-1652`. It does not change a
Maestro product contract and it does not treat the historical V3 Stage 12 chain
or any Stage 0/2/3/4/5 verdict as current proof.

`materialize_chain.py` is the deterministic, no-ref-update helper the external
Orchestrator runs only after promotion. It writes Git commit objects for one
synthetic chain: the fixed historical Stage 0-4 checkpoints, a Stage 5 merge,
single-parent Stage 6-11 checkpoints, and a Stage 12 merge whose second parent
is the exact reviewed promotion candidate and whose tree is identical. It also
writes the exact byte-total Stage 12 overlay manifest. It never updates a ref,
generates a closure, seals, or publishes.

`generate.py` is the only generation entry point. It refuses a dirty candidate,
binds the exact clean `--final-ref` commit and tree as the current V4 Stage 12
checkpoint, verifies the exact merge/direct-parent topology and all thirteen
supplied checkpoint trees, archives that commit without reading mutable
working-tree bytes, and copies/content-binds the approved V4 packet. It then
emits a byte-total input manifest, a ledger derived without classification
inference from `proof-registry.v1.json`, semantic artifact-readback requirements,
three complete per-engine Cargo/native dependency closures, checkpoint records,
an exact reachable Git-object pack for ancestry proof, and the immutable
snapshot. The runner indexes that bound pack into a separate read-only bare
repository for each engine; ancestry proof never points at the archive-only
source tree. Generation does not execute proof, write a receipt, update a
pointer, or publish.

The Orchestrator supplies exactly one `--stage-checkpoint N=<40-hex-commit>` for
every `N` from 0 through 12. Stage 12 must equal the commit resolved from
`--final-ref`; no historical V3 commit is accepted in that slot.
Generation additionally requires the exact reviewed Stage 12 candidate, the
helper-produced overlay manifest, and a byte-bound promotion-prerequisites
receipt. The receipt must prove the legacy prune gate is zero, consumer/reader/
hold counts are zero, promotion parity is exactly 210/210 with zero mismatches,
and the four resolved Stage 11/12 exact lib filters match the rotated registry.
The current pre-promotion observation is byte-bound by the Stage 12 validator
at 384 legacy rows. Generation and sealing remain blocked until a fresh
post-promotion receipt proves the exact zero-legacy, zero-consumer, zero-reader,
and zero-hold closure.

The Stage 10 ownership proof runs `stage12_product_proof.py` against the frozen
source snapshot and isolated ancestry repository. It proves the exact
Stage12Product correction checkpoint is an ancestor of the reviewed candidate,
proves that candidate is the final integration's direct second parent with the
same tree, and validates the complete reviewed-candidate history rather than
truncating proof at the correction checkpoint.

The Stage 12 coordinator root must carry the approved packet-closure file
`control/stage12/packet/protected-primary-binding.v7.1.json`. Generation copies
and identity-checks that file into the frozen closure. The runner independently
reconstructs the protected primary's commit, tree, ordered dirty-path manifest,
tracked binary diff, and untracked regular-file path/mode/length/SHA manifest
before creating the run root, immediately before engines, and immediately
before publication. Engines never receive protected-primary filesystem access.

```text
python3 tools/vnext_contracts/final_chain/generate.py \
  --repository <clean-isolated-final-V4-worktree> \
  --packet-root /private/tmp/maestro-vnext-final-closure-successor-packet-v4 \
  --final-ref <exact-final-V4-commit> \
  --stage12-reviewed-candidate <exact-reviewed-promotion-candidate> \
  --stage12-overlay-manifest <helper-produced-overlay-manifest> \
  --promotion-prerequisites <zero-gate-byte-bound-prerequisites-receipt> \
  --stage-checkpoint 0=<stage-0-first-parent-checkpoint> \
  --stage-checkpoint 1=<stage-1-first-parent-checkpoint> \
  --stage-checkpoint 2=<stage-2-first-parent-checkpoint> \
  --stage-checkpoint 3=<stage-3-first-parent-checkpoint> \
  --stage-checkpoint 4=<stage-4-first-parent-checkpoint> \
  --stage-checkpoint 5=<stage-5-noncertifying-checkpoint> \
  --stage-checkpoint 6=<stage-6-integrated-checkpoint> \
  --stage-checkpoint 7=<stage-7-integrated-checkpoint> \
  --stage-checkpoint 8=<stage-8-integrated-checkpoint> \
  --stage-checkpoint 9=<stage-9-integrated-checkpoint> \
  --stage-checkpoint 10=<stage-10-integrated-checkpoint> \
  --stage-checkpoint 11=<stage-11-integrated-checkpoint> \
  --stage-checkpoint 12=<exact-final-V4-commit> \
  --output-root <new-disjoint-frozen-closure-root> \
  --publication-root <dedicated-final-publication-root> \
  --protected-primary /Users/reinamaccredy/Code/maestro \
  --target aarch64-apple-darwin \
  --profile test-unoptimized
```

`runner.py` is the one later seal. It refuses unless closure, run, publication,
and protected-primary roots are disjoint; every frozen byte and exact tool probe
still matches; `/usr/bin/sandbox-exec` is available; and active probes prove
network, protected-primary reads and writes, and immutable-root writes are
denied. Each engine receives a separate read-only source copy and separate temp,
target, dependency, and output roots. Cargo proof commands are frozen and
offline. There is no sandbox fallback. Because Codex itself runs inside a
sandbox, the Orchestrator must launch this one command through the approved
unsandboxed execution boundary; nested sandbox application failure is a refusal,
not a reason to weaken the profile.

```text
python3 tools/vnext_contracts/final_chain/runner.py \
  --closure-root <frozen-closure-root> \
  --run-root <new-disjoint-run-root> \
  --publication-root <same-dedicated-final-publication-root> \
  --sandbox-exec /usr/bin/sandbox-exec
```

Fault, crash-replay, migration, rollback, ancestry, and Stage 12 checks must
write their typed receipts to the paths supplied in
`MAESTRO_FINAL_PROOF_RECEIPT` or
`MAESTRO_SEMANTIC_READBACK_RECEIPT`. Scheduling metadata, source substring
counts, and constant pass markers are not observations. Fault points and
migration routes each bind a separately emitted observation file; cohort
executable identities bind actual source, target, or output bytes. The semantic
receipts bind actual compiled/exported/schema/resource/persisted/consumer/
reader/hold artifact bytes plus separately emitted canonical-read and
negative-route observations, and zero consumer/reader/hold closure.

The runner publishes one immutable release object only after typed row-for-row
three-engine consensus and the independently executed 12-edge ancestry sweep.
The pre-created publication root is bound by path, device, inode, and mount
custody. A pre-existing object must match every byte. The single `current.json`
pointer advances by the snapshot's exact preimage and monotonic generation CAS,
with descriptor-relative no-follow writes and durable file/directory syncs. No
source generation or seal has been run by this candidate commit.
