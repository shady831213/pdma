use super::endpoint::Endpoint;
use super::operations::{
    AtomicCommand, FillCommand, SemaphoreCommand, TimestampCommand, WaitCommand, WordWrite,
};
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Independent linear or scatter/gather copy"]
    #[description = "Independent linear or scatter/gather copy."]
    pub record CopyCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Source stream and start"]
            #[description = "Source stream and start."]
            source: Endpoint,
            #[short_description = "Destination stream and start"]
            #[description = "Destination stream and start."]
            destination: Endpoint,
            #[short_description = "Selected byte count; both ranges are validated before writes"]
            #[description = "Selected byte count; both ranges are validated before writes."]
            copy_length: u64,
            #[short_description = "Context shared by source and destination SG tables"]
            #[description = "Context shared by source and destination SG tables."]
            sg_context: u32,
        },
    }
}
record_schema! {
    #[short_description = "Fixed-size copy-engine command"]
    #[description = "Fixed-size copy-engine command. Successful work produces no event."]
    pub record Command {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Software identifier retained in error events"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "Operation selected by the payload tag"]
            #[description = "Operation selected by the payload tag."]
            kind: enum(u32) { variants: { Copy = 0, Write = 1, Fill = 2, Wait = 3, Atomic = 4, Semaphore = 5, Fence = 6, Trap = 7, Timestamp = 8 } },
            #[short_description = "Only the selected operation payload is meaningful; inactive bytes are zero"]
            #[description = "Only the selected operation payload is meaningful; inactive bytes are zero."]
            payload: union { tag: self.kind, cases: { Copy(CopyCommand), Write(WordWrite), Fill(FillCommand), Wait(WaitCommand), Atomic(AtomicCommand), Semaphore(SemaphoreCommand), Fence(WordWrite), Trap(TrapCommand), Timestamp(TimestampCommand) } },
        },
    }
}

impl Command {
    #[doc = "Constructs an external timer sampling command."]
    pub const fn timestamp(id: u64, payload: TimestampCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Timestamp,
            payload: CommandPayloadValues::Timestamp(payload),
        }
    }
    #[doc = "Constructs a Copy command."]
    pub const fn copy(id: u64, payload: CopyCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Copy,
            payload: CommandPayloadValues::Copy(payload),
        }
    }
    #[doc = "Constructs a Write command."]
    pub const fn write(id: u64, payload: WordWrite) -> Self {
        Self {
            id,
            kind: CommandKind::Write,
            payload: CommandPayloadValues::Write(payload),
        }
    }
    #[doc = "Constructs a Fill command."]
    pub const fn fill(id: u64, payload: FillCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Fill,
            payload: CommandPayloadValues::Fill(payload),
        }
    }
    #[doc = "Constructs a Wait command."]
    pub const fn wait(id: u64, payload: WaitCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Wait,
            payload: CommandPayloadValues::Wait(payload),
        }
    }
    #[doc = "Constructs a Atomic command."]
    pub const fn atomic(id: u64, payload: AtomicCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Atomic,
            payload: CommandPayloadValues::Atomic(payload),
        }
    }
    #[doc = "Constructs a Semaphore command."]
    pub const fn semaphore(id: u64, payload: SemaphoreCommand) -> Self {
        Self {
            id,
            kind: CommandKind::Semaphore,
            payload: CommandPayloadValues::Semaphore(payload),
        }
    }
    #[doc = "Constructs a Fence command."]
    pub const fn fence(id: u64, payload: WordWrite) -> Self {
        Self {
            id,
            kind: CommandKind::Fence,
            payload: CommandPayloadValues::Fence(payload),
        }
    }
    #[doc = "Constructs a Trap notification command."]
    pub const fn trap(id: u64) -> Self {
        Self {
            id,
            kind: CommandKind::Trap,
            payload: CommandPayloadValues::Trap(TrapCommand {}),
        }
    }
}

record_schema! {
    #[short_description = "Empty Trap payload; notification only"]
    #[description = "Empty Trap payload; notification only."]
    pub record TrapCommand { alignment: 8, endian: Little, fields: {} }
}
