use super::{Command, CommandCase, CommandHeader, CommandRaw, CommandView, CommandViewMut, Opcode};
use hardware_interface_schema::{Decode, Encode, RecordLayout, SchemaCodecError};

impl Command {
    /// Decodes the common header and selects the complete command record.
    /// # Errors
    /// Rejects short storage, unknown opcode, reserved storage or invalid operands.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        let case = match CommandHeader::from_bytes(bytes)?.opcode {
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

    /// Encodes a complete command after checking its shared header.
    /// # Errors
    /// Rejects an opcode inconsistent with the selected command or invalid operands.
    pub fn to_bytes(self) -> Result<[u8; Self::RECORD_SIZE], SchemaCodecError> {
        let mut bytes = [0; Self::RECORD_SIZE];
        self.encode_into(&mut bytes)?;
        Ok(bytes)
    }

    /// Writes a complete protocol-valid command to caller storage.
    /// # Errors
    /// Rejects inconsistent opcodes, invalid operands or short storage.
    pub fn encode_into(&self, bytes: &mut [u8]) -> Result<(), SchemaCodecError> {
        let (observed, expected) = match self {
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
                location: CommandHeader::OPCODE_IDENTITY.at_byte(0),
                observed: u64::from(observed.raw()),
                expected: u64::from(expected.raw()),
            });
        }
        self.encode_variant(bytes)
    }

    /// Software identifier of the selected complete command.
    pub const fn id(&self) -> u64 {
        match self {
            Self::Copy(value) => value.id,
            Self::Write(value) => value.id,
            Self::Fill(value) => value.id,
            Self::Wait(value) => value.id,
            Self::Atomic(value) => value.id,
            Self::Semaphore(value) => value.id,
            Self::Fence(value) => value.id,
            Self::Trap(value) => value.id,
            Self::Timestamp(value) => value.id,
        }
    }
}

impl Encode for Command {
    type Encoded = CommandRaw;
    fn encode(self) -> Result<Self::Encoded, SchemaCodecError> {
        self.to_bytes().map(CommandRaw::new)
    }
}
impl Decode for CommandRaw {
    type Decoded = Command;
    fn decode(self) -> Result<Command, SchemaCodecError> {
        Command::from_bytes(self.as_ref())
    }
}
impl RecordLayout for Command {
    type Value = Self;
    type View<'a> = CommandView<'a>;
    type ViewMut<'a> = CommandViewMut<'a>;
    const NAME: &'static str = "Command";
    const DOCUMENTATION: hardware_interface_schema::Documentation = Self::SPEC.documentation();
    const ALIGNMENT: usize = Self::RECORD_ALIGNMENT;
    const EXTENT: usize = Self::RECORD_SIZE;
    const ZERO: Self = Self::ZERO;
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::from_bytes(bytes)
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_into(bytes)
    }
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> {
        CommandView::new(bytes)
    }
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        CommandViewMut::new(bytes)
    }
}
