use std::collections::BTreeSet;

use thiserror::Error;

use super::{
    DistributionDomainRefV1, DistributionModelErrorV1, DistributionRuntimeObjectKindV1,
    DistributionScopedObjectRefV1,
};

pub const ORDINARY_SNAPSHOT_CAPACITY_V1: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinarySnapshotStateV1 {
    Eligible,
    Tombstoned,
    Erased,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedSnapshotHoldV1 {
    pub snapshot_ref: DistributionScopedObjectRefV1,
    pub retention_pin_ref: DistributionScopedObjectRefV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupDebtV1 {
    pub snapshot_ref: DistributionScopedObjectRefV1,
    pub cleanup_debt_ref: DistributionScopedObjectRefV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRotationV1 {
    pub captured_prior_ref: DistributionScopedObjectRefV1,
    pub source_commit_ref: DistributionScopedObjectRefV1,
    pub captured_sequence: u64,
    pub selected_rollback_ref: Option<DistributionScopedObjectRefV1>,
    pub committed_current_ref: DistributionScopedObjectRefV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinarySnapshotCatalogStateV1 {
    domain: DistributionDomainRefV1,
    excluded_current_state_ref: DistributionScopedObjectRefV1,
    eligible: Vec<(
        DistributionScopedObjectRefV1,
        DistributionScopedObjectRefV1,
        u64,
    )>,
    protected_holds: Vec<ProtectedSnapshotHoldV1>,
    cleanup_debts: Vec<CleanupDebtV1>,
    invalidated: BTreeSet<DistributionScopedObjectRefV1>,
}

impl OrdinarySnapshotCatalogStateV1 {
    pub fn empty(
        domain: DistributionDomainRefV1,
        current_state_ref: DistributionScopedObjectRefV1,
    ) -> Result<Self, SnapshotCatalogErrorV1> {
        current_state_ref.require_same_domain(&domain)?;
        current_state_ref
            .require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
        Ok(Self {
            domain,
            excluded_current_state_ref: current_state_ref,
            eligible: Vec::new(),
            protected_holds: Vec::new(),
            cleanup_debts: Vec::new(),
            invalidated: BTreeSet::new(),
        })
    }

    pub const fn domain(&self) -> &DistributionDomainRefV1 {
        &self.domain
    }

    pub const fn excluded_current_state_ref(&self) -> &DistributionScopedObjectRefV1 {
        &self.excluded_current_state_ref
    }

    pub fn eligible(
        &self,
    ) -> &[(
        DistributionScopedObjectRefV1,
        DistributionScopedObjectRefV1,
        u64,
    )] {
        &self.eligible
    }

    pub fn protected_holds(&self) -> &[ProtectedSnapshotHoldV1] {
        &self.protected_holds
    }

    pub fn cleanup_debts(&self) -> &[CleanupDebtV1] {
        &self.cleanup_debts
    }

    pub fn rotate_after_commit(
        &mut self,
        rotation: CatalogRotationV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        self.validate_rotation(&rotation)?;
        let selected = rotation.selected_rollback_ref.as_ref();
        let mut next = vec![(
            rotation.captured_prior_ref.clone(),
            rotation.source_commit_ref,
            rotation.captured_sequence,
        )];
        next.extend(
            self.eligible
                .iter()
                .filter(|(snapshot, _, _)| Some(snapshot) != selected)
                .filter(|(snapshot, _, _)| snapshot != &rotation.captured_prior_ref)
                .filter(|(snapshot, _, _)| snapshot != &rotation.committed_current_ref)
                .filter(|(snapshot, _, _)| !self.invalidated.contains(snapshot))
                .cloned(),
        );
        next.truncate(ORDINARY_SNAPSHOT_CAPACITY_V1);
        self.eligible = next;
        self.excluded_current_state_ref = rotation.committed_current_ref;
        Ok(())
    }

    pub fn add_protected_hold(
        &mut self,
        hold: ProtectedSnapshotHoldV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        self.validate_snapshot_ref(&hold.snapshot_ref)?;
        hold.retention_pin_ref.require_same_domain(&self.domain)?;
        hold.retention_pin_ref
            .require_kind(DistributionRuntimeObjectKindV1::RetentionPinSet)?;
        if self
            .protected_holds
            .iter()
            .any(|existing| existing.snapshot_ref == hold.snapshot_ref)
        {
            return Err(SnapshotCatalogErrorV1::DuplicateSnapshot);
        }
        self.protected_holds.push(hold);
        self.protected_holds
            .sort_by_key(|item| item.snapshot_ref.object_id());
        Ok(())
    }

    pub fn record_cleanup_debt(
        &mut self,
        debt: CleanupDebtV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        self.validate_snapshot_ref(&debt.snapshot_ref)?;
        debt.cleanup_debt_ref.require_same_domain(&self.domain)?;
        debt.cleanup_debt_ref
            .require_kind(DistributionRuntimeObjectKindV1::CleanupDebtSet)?;
        if self
            .cleanup_debts
            .iter()
            .any(|existing| existing.snapshot_ref == debt.snapshot_ref)
        {
            return Err(SnapshotCatalogErrorV1::DuplicateSnapshot);
        }
        self.cleanup_debts.push(debt);
        self.cleanup_debts
            .sort_by_key(|item| item.snapshot_ref.object_id());
        Ok(())
    }

    pub fn invalidate(
        &mut self,
        snapshot_ref: DistributionScopedObjectRefV1,
        state: OrdinarySnapshotStateV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        self.validate_snapshot_ref(&snapshot_ref)?;
        if state == OrdinarySnapshotStateV1::Eligible {
            return Err(SnapshotCatalogErrorV1::InvalidInvalidationState);
        }
        self.invalidated.insert(snapshot_ref.clone());
        self.eligible
            .retain(|(eligible, _, _)| eligible != &snapshot_ref);
        Ok(())
    }

    pub fn selectable(
        &self,
        snapshot_ref: &DistributionScopedObjectRefV1,
    ) -> Result<bool, SnapshotCatalogErrorV1> {
        self.validate_snapshot_ref(snapshot_ref)?;
        Ok(!self.invalidated.contains(snapshot_ref)
            && self
                .eligible
                .iter()
                .any(|(eligible, _, _)| eligible == snapshot_ref))
    }

    fn validate_rotation(
        &self,
        rotation: &CatalogRotationV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        self.validate_snapshot_ref(&rotation.captured_prior_ref)?;
        rotation
            .source_commit_ref
            .require_same_domain(&self.domain)?;
        rotation
            .source_commit_ref
            .require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
        rotation
            .committed_current_ref
            .require_same_domain(&self.domain)?;
        rotation
            .committed_current_ref
            .require_kind(DistributionRuntimeObjectKindV1::DistributionCommitRecord)?;
        if rotation.captured_sequence == 0
            || rotation.captured_prior_ref == rotation.committed_current_ref
            || rotation.source_commit_ref != self.excluded_current_state_ref
        {
            return Err(SnapshotCatalogErrorV1::InvalidRotation);
        }
        if let Some(selected) = &rotation.selected_rollback_ref
            && !self.selectable(selected)?
        {
            return Err(SnapshotCatalogErrorV1::RollbackSnapshotNotSelectable);
        }
        Ok(())
    }

    fn validate_snapshot_ref(
        &self,
        snapshot_ref: &DistributionScopedObjectRefV1,
    ) -> Result<(), SnapshotCatalogErrorV1> {
        snapshot_ref.require_same_domain(&self.domain)?;
        snapshot_ref.require_kind(DistributionRuntimeObjectKindV1::DistributionSnapshot)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SnapshotCatalogErrorV1 {
    #[error("ordinary snapshot catalog rotation is not based on the current committed state")]
    InvalidRotation,
    #[error("rollback requires an eligible ordinary snapshot in the same domain")]
    RollbackSnapshotNotSelectable,
    #[error("snapshot catalog repeats an independently accounted snapshot")]
    DuplicateSnapshot,
    #[error("only tombstoned or erased snapshots can lose selectability")]
    InvalidInvalidationState,
    #[error(transparent)]
    DistributionModel(#[from] DistributionModelErrorV1),
}
