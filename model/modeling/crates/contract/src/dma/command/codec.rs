use super::{Command, CommandCase, CommandHeader, Opcode};
use hw_model_schema::{SchemaCodec, SchemaCodecError};

impl SchemaCodec for Command {
    /// Decodes the common header and selects the complete command record.
    /// # Errors
    /// Rejects short storage, unknown opcode, reserved storage or invalid operands.
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        let case = match <CommandHeader as SchemaCodec>::decode(bytes)?.opcode {
            Opcode::Copy => CommandCase::Copy,
            Opcode::Write => CommandCase::Write,
            Opcode::Fill => CommandCase::Fill,
            Opcode::Wait => CommandCase::Wait,
            Opcode::Atomic => CommandCase::Atomic,
            Opcode::Semaphore => CommandCase::Semaphore,
            Opcode::Fence => CommandCase::Fence,
            Opcode::Trap => CommandCase::Trap,
            Opcode::Timestamp => CommandCase::Timestamp,
        };
        Self::decode_variant(bytes, case)
    }
    /// Writes a complete protocol-valid command to caller storage.
    /// # Errors
    /// Rejects inconsistent opcodes, invalid operands or short storage.
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        let (observed, expected) = match value {
            Self::Copy(value) => (value.header.opcode, Opcode::Copy),
            Self::Write(value) => (value.header.opcode, Opcode::Write),
            Self::Fill(value) => (value.header.opcode, Opcode::Fill),
            Self::Wait(value) => (value.header.opcode, Opcode::Wait),
            Self::Atomic(value) => (value.header.opcode, Opcode::Atomic),
            Self::Semaphore(value) => (value.header.opcode, Opcode::Semaphore),
            Self::Fence(value) => (value.header.opcode, Opcode::Fence),
            Self::Trap(value) => (value.header.opcode, Opcode::Trap),
            Self::Timestamp(value) => (value.header.opcode, Opcode::Timestamp),
        };
        if observed != expected {
            return Err(SchemaCodecError::ConstantMismatch {
                location: CommandHeader::OPCODE_IDENTITY.at_byte(CommandHeader::OPCODE_OFFSET),
                observed: u64::from(observed.raw()),
                expected: u64::from(expected.raw()),
            });
        }
        value.encode_variant(bytes)
    }
}
