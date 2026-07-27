//! Projection-owned Packet, frontier, recommendation, and replay facade.

mod engine;

#[allow(
    unused_imports,
    reason = "the canonical projection facade keeps implementation children private"
)]
pub(crate) use engine::{
    CANONICAL_PACKET_READ_REPLACEMENT_V1, LegacySuccessorRefusalV1, LegacySuccessorSurfaceV1,
    ProjectionErrorV1, ProjectionReadPortV1, ProjectionReadStateV1, ProjectionSnapshotV1,
    RecipeComponentProjectionV1, UNSUPPORTED_LEGACY_SUCCESSOR_SURFACE_V1, packet_semantic_hash,
    read_packet, refuse_legacy_successor_surface,
};
