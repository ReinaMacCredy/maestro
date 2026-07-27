//! Projection-owned Packet, frontier, recommendation, and replay facade.

mod engine;

#[allow(
    unused_imports,
    reason = "the canonical projection facade keeps implementation children private"
)]
pub(crate) use engine::{
    ProjectionErrorV1, ProjectionReadPortV1, ProjectionReadStateV1, ProjectionSnapshotV1,
    RecipeComponentProjectionV1, packet_semantic_hash, read_packet,
};
