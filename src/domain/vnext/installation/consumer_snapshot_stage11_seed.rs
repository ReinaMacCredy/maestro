use std::marker::PhantomData;

#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
use sha2::{Digest, Sha256};

use super::consumer_snapshot::{
    ConsumerClosureDurableLinearizationRequestV1, ConsumerClosureDurableLinearizationV1,
    ConsumerClosureStageV1, InstallationConsumerSnapshotErrorV1,
};

// Stage 11 owns the backend replacement in this inherited seed. The frozen
// Installation facade and opaque operation remain unchanged.
enum Stage11ConsumerClosureDurableEffectBackendV1 {
    Unavailable,
    #[cfg(test)]
    Conformance {
        effects: Rc<Cell<u64>>,
        durable_association: Rc<Cell<[u8; 32]>>,
        apply_effect: bool,
    },
}

pub(super) struct Stage11ConsumerClosureDurableEffectSeedV1<K> {
    backend: Stage11ConsumerClosureDurableEffectBackendV1,
    _stage: PhantomData<K>,
}

pub(super) struct Stage11ConsumerClosureAppliedEffectV1<K> {
    expected_old_cas: [u8; 32],
    consumer_set_id: [u8; 32],
    durable_effect_commitment: [u8; 32],
    _stage: PhantomData<K>,
}

impl<K> Stage11ConsumerClosureAppliedEffectV1<K> {
    pub(super) fn into_commitments(self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            self.expected_old_cas,
            self.consumer_set_id,
            self.durable_effect_commitment,
        )
    }
}

impl<K: ConsumerClosureStageV1> Stage11ConsumerClosureDurableEffectSeedV1<K> {
    pub(super) fn commit(
        self,
        request: &ConsumerClosureDurableLinearizationRequestV1<K>,
    ) -> Result<Stage11ConsumerClosureAppliedEffectV1<K>, InstallationConsumerSnapshotErrorV1> {
        #[cfg(not(test))]
        let _ = request;
        match self.backend {
            Stage11ConsumerClosureDurableEffectBackendV1::Unavailable => {
                Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch)
            }
            #[cfg(test)]
            Stage11ConsumerClosureDurableEffectBackendV1::Conformance {
                effects,
                durable_association,
                apply_effect,
            } => {
                let effect_commitment = durable_effect_commitment(
                    K::TAG,
                    request.expected_old_cas(),
                    request.consumer_set_id(),
                );
                if apply_effect {
                    durable_association.set(effect_commitment);
                    effects.set(effects.get() + 1);
                }
                if durable_association.get() != effect_commitment {
                    return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
                }
                Ok(Stage11ConsumerClosureAppliedEffectV1 {
                    expected_old_cas: request.expected_old_cas(),
                    consumer_set_id: request.consumer_set_id(),
                    durable_effect_commitment: effect_commitment,
                    _stage: PhantomData,
                })
            }
        }
    }
}

#[cfg(test)]
fn durable_effect_commitment(
    stage: u8,
    expected_old_cas: [u8; 32],
    consumer_set_id: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"maestro.vnext.consumer-closure-durable-effect.v1\0");
    hasher.update([stage]);
    hasher.update(expected_old_cas);
    hasher.update(consumer_set_id);
    hasher.finalize().into()
}

pub(super) fn acquire<K: ConsumerClosureStageV1>()
-> Result<ConsumerClosureDurableLinearizationV1<K>, InstallationConsumerSnapshotErrorV1> {
    Ok(
        ConsumerClosureDurableLinearizationV1::from_stage11_owner_seed(
            Stage11ConsumerClosureDurableEffectSeedV1 {
                backend: Stage11ConsumerClosureDurableEffectBackendV1::Unavailable,
                _stage: PhantomData,
            },
        ),
    )
}

#[cfg(test)]
pub(in crate::domain::vnext) mod test_seed {
    use super::*;

    pub fn successful<K: ConsumerClosureStageV1>(
        effects: Rc<Cell<u64>>,
    ) -> ConsumerClosureDurableLinearizationV1<K> {
        conformance(effects, true)
    }

    pub fn no_effect<K: ConsumerClosureStageV1>(
        effects: Rc<Cell<u64>>,
    ) -> ConsumerClosureDurableLinearizationV1<K> {
        conformance(effects, false)
    }

    fn conformance<K: ConsumerClosureStageV1>(
        effects: Rc<Cell<u64>>,
        apply_effect: bool,
    ) -> ConsumerClosureDurableLinearizationV1<K> {
        ConsumerClosureDurableLinearizationV1::from_stage11_owner_seed(
            Stage11ConsumerClosureDurableEffectSeedV1 {
                backend: Stage11ConsumerClosureDurableEffectBackendV1::Conformance {
                    effects,
                    durable_association: Rc::new(Cell::new([0; 32])),
                    apply_effect,
                },
                _stage: PhantomData,
            },
        )
    }
}
