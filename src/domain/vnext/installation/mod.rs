//! Installation transaction, custody, recovery, and currentness implementation seam.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Installation-owned consumer snapshot before its Stage 9 through 11 consumers"
    )
)]
pub(in crate::domain::vnext) mod consumer_snapshot;
mod consumer_snapshot_stage11_seed;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes durable Installation finality before its Stage 9 and Stage 11 production consumers"
    )
)]
pub(in crate::domain::vnext) mod durable_finality;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the PreStore owner seed before Stage 11 integrates it"
    )
)]
mod durable_finality_stage11_seed;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the ActiveStore owner seed before Stage 9 integrates it"
    )
)]
mod durable_finality_stage9_seed;
