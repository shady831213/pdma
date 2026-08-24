use crate::abi::{AbiEncodeError, codec::encode_schema_error};

use super::AbiCommand;

pub(super) fn encode_descriptor(
    command: AbiCommand,
    output: &mut [u8],
) -> Result<(), AbiEncodeError> {
    command
        .values()
        .encode_into(output)
        .map_err(encode_schema_error)
}
