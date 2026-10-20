use super::endpoint::Endpoint;
use hardware_interface_schema::{bits_schema, record_schema};

pub(super) mod signal;
pub(super) mod wait;
use signal::SignalCommand;
use wait::WaitCommand;

bits_schema! {
    pub bits CompletionFlags(u32) {
        #[short_description = "Interrupt on completion"]
        #[description = "Request a channel notification after successful response publication, independently of empty-to-non-empty notification. Echoed in the response; consuming that response clears its IOC cause. Device masks suppress new notifications without replay on unmask."]
        ioc: 0..=0,
    }
}

record_schema! {
    #[short_description = "Copy parameters"]
    #[description = "Two independent endpoint starts select one copy_length. Hardware validates both ranges before writes. Software retains lists and resources until completion or acknowledged cancellation, including fault suspension."]
    pub record CopyCommand {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Source stream"]
            #[description = "Linear or scatter/gather stream supplying payload bytes in logical order."]
            source: Endpoint,
            #[short_description = "Destination stream"]
            #[description = "Independent destination stream receiving copy_length bytes from its selected start."]
            destination: Endpoint,
            #[short_description = "Copy byte length"]
            #[description = "Bytes requested from each endpoint start. Zero performs no list or payload accesses. Each selected SG range must cover this many bytes; insufficient capacity reports LengthMismatch before payload writes."]
            copy_length: u64,
            #[short_description = "Scatter/gather table context"]
            #[description = "Context selector shared by both scatter/gather tables. Entries describe payload in each endpoint's own context. Ignored when both endpoints are linear or hardware is context-free."]
            sg_context: u32,
        },
    }
}

record_schema! {
    #[short_description = "DMA command"]
    #[description = "An eight-byte-aligned little-endian tagged Copy, Signal or Wait submission. Software retains resources and context bindings until terminal completion or acknowledged cancellation."]
    pub record Command {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Command identifier"]
            #[description = "Software-selected identifier echoed unchanged in the response."]
            id: u64,
            #[short_description = "Completion notification flags"]
            #[description = "IOC requests notification after response publication. Reserved bits must be zero."]
            flags: bits(CompletionFlags),
            #[short_description = "Command operation"]
            #[description = "Copy is zero, Signal is one and Wait is two. The payload case determines the encoded operation."]
            opcode: enum(u32) { variants: { Copy = 0, Signal = 1, Wait = 2 } },
            #[short_description = "Operation parameters"]
            #[description = "The opcode selects the only active parameter record; unused union bytes are reserved."]
            payload: union {
                tag: self.opcode,
                cases: { Copy(CopyCommand), Signal(SignalCommand), Wait(WaitCommand) },
            },
        },
    }
}

impl Command {
    /// Creates a copy command without an IOC request.
    pub const fn copy(id: u64, copy: CopyCommand) -> Self {
        Self {
            id,
            flags: CompletionFlagsValues { ioc: false },
            opcode: CommandOpcode::Copy,
            payload: CommandPayloadValues::Copy(copy),
        }
    }

    /// Creates an atomic signal command without an IOC request.
    pub const fn signal(id: u64, signal: SignalCommand) -> Self {
        Self {
            id,
            flags: CompletionFlagsValues { ioc: false },
            opcode: CommandOpcode::Signal,
            payload: CommandPayloadValues::Signal(signal),
        }
    }

    /// Creates a signal wait command without an IOC request.
    pub const fn wait(id: u64, wait: WaitCommand) -> Self {
        Self {
            id,
            flags: CompletionFlagsValues { ioc: false },
            opcode: CommandOpcode::Wait,
            payload: CommandPayloadValues::Wait(wait),
        }
    }

    /// Borrows copy parameters when this command carries a copy.
    pub const fn as_copy(&self) -> Option<&CopyCommand> {
        match &self.payload {
            CommandPayloadValues::Copy(copy) => Some(copy),
            CommandPayloadValues::Signal(_) | CommandPayloadValues::Wait(_) => None,
        }
    }

    /// Mutably borrows copy parameters when this command carries a copy.
    pub const fn as_copy_mut(&mut self) -> Option<&mut CopyCommand> {
        match &mut self.payload {
            CommandPayloadValues::Copy(copy) => Some(copy),
            CommandPayloadValues::Signal(_) | CommandPayloadValues::Wait(_) => None,
        }
    }
}
