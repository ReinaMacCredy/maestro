//! Canonical Packet, Operation, Result, and Action transport facade.

mod json;

#[allow(
    unused_imports,
    reason = "Stage 6 preserves its frozen candidate facade before root integration"
)]
pub(crate) use json::{
    TransportErrorV1, decode_operation_request, decode_packet_read_request,
    encode_operation_request, encode_operation_result, encode_packet_read_envelope,
    encode_packet_read_request, encoded_packet_len,
};
