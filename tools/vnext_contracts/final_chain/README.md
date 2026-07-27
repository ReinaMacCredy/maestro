# Final cumulative proof runner

This namespace is the V4 source-only candidate for the single Stage 0 through
12 final seal. It accepts explicit frozen inputs and has no default input,
output, receipt, or pointer path.

The runner makes a separate read-only materialization for Python, Rust, and
Ruby. Each interpreter parses the snapshot, proof ledger, and Stage 12 semantic
readback plan itself, executes all assigned commands, and writes only an
ephemeral engine receipt. The runner accepts the three complete receipts only
when every proof ID and expected outcome matches, then writes one
content-addressed release object and one pointer in the caller-provided proof
root.

The current checkout contains schemas, hostile fixtures, and static checks only.
No frozen final snapshot, proof ledger, engine receipt, release object, or
pointer is supplied or generated here.
