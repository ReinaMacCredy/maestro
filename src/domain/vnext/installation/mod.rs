//! Installation transaction, custody, recovery, and currentness implementation seam.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Installation-owned consumer snapshot before its Stage 9 through 11 consumers"
    )
)]
pub(in crate::domain::vnext) mod consumer_snapshot;
