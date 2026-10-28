use super::command::CommandHeader;
use super::endpoint::{LinearEndpoint, ScatterGatherEndpoint};
mod write;
pub use write::*;
mod atomic;
pub use atomic::{
    AtomicCommand, AtomicCommandOperandsAddValues, AtomicCommandOperandsAndValues,
    AtomicCommandOperandsCompareExchangeValues, AtomicCommandOperandsExchangeValues,
    AtomicCommandOperandsOrValues, AtomicCommandOperandsRaw, AtomicCommandOperandsSubtractValues,
    AtomicCommandOperandsValues, AtomicCommandOperandsXorValues, AtomicCommandOperation,
    AtomicCommandOperationRaw, AtomicCommandOrder, AtomicCommandOrderRaw, AtomicCommandRaw,
    AtomicCommandView, AtomicCommandViewMut,
};
use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "Complete independent-endpoint copy command"]
    #[description = "A 96-byte command with independently selected linear or scatter/gather endpoints; both ranges are validated before writes."]
    pub record CopyCommand { alignment: 8, endian: Little, fields: {
        #[short_description = "Shared command header"] #[description = "Copy opcode in the shared header."]
        header: CommandHeader,
        #[short_description = "Independent endpoint selectors"] #[description = "Source and destination selectors; other bits are reserved zero."]
        options: bits(u8) { fields: {
            #[short_description = "Source representation"] #[description = "Linear zero or scatter/gather one."]
            source_mode: 0..=0 => enum { variants: { Linear = 0, ScatterGather = 1 } },
            #[short_description = "Destination representation"] #[description = "Linear zero or scatter/gather one."]
            destination_mode: 1..=1 => enum { variants: { Linear = 0, ScatterGather = 1 } },
        } },
        #[short_description = "List address space"] #[description = "Context shared by source and destination scatter/gather tables."]
        sg_context: u32,
        #[short_description = "Software identifier"] #[description = "Identifier retained in error events."]
        id: u64,
        #[short_description = "Copy byte count"] #[description = "Selected full-width byte count."]
        copy_length: u64,
        #[short_description = "Source payload context"] #[description = "Source payload address-space selector."]
        source_context: u32,
        #[short_description = "Destination payload context"] #[description = "Destination payload address-space selector."]
        destination_context: u32,
        #[short_description = "Source stream"] #[description = "Source geometry and start; inactive bytes are zero."]
        source: tagged_union { tag: self.options.source_mode, cases: { Linear(LinearEndpoint), ScatterGather(ScatterGatherEndpoint) } },
        #[short_description = "Destination stream"] #[description = "Destination geometry and start; inactive bytes are zero."]
        destination: tagged_union { tag: self.options.destination_mode, cases: { Linear(LinearEndpoint), ScatterGather(ScatterGatherEndpoint) } },
    } }
}
record_schema! {
    #[short_description = "External timer sample destination"]
    #[description = "Writes one little-endian 64-bit external timer sample. A target PageFault retains the original sample until resume or cancellation; units and timer progression belong to the platform."]
    pub record TimestampCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "Eight-byte-aligned destination address"]
            #[description = "Eight-byte-aligned destination address for one 64-bit timer sample."]
            address: u64,
            #[short_description = "Destination address-space selector"]
            #[description = "Destination context, authorized through the ordinary DMA memory path and retained through command retirement."]
            context: u32,
        },
    }
}

record_schema! {
    #[short_description = "Shared scalar target"]
    #[description = "Shared scalar target."]
    pub record WordTarget {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Naturally aligned little-endian word address"]
            #[description = "Naturally aligned little-endian word address."]
            address: u64,
            #[short_description = "Target address-space selector"]
            #[description = "Target address-space selector."]
            context: u32,
            #[short_description = "Word width; values and operands truncate to that width"]
            #[description = "Word width; values and operands truncate to that width."]
            width: enum(u32) { variants: { Bits32 = 0, Bits64 = 1 } },
        },
    }
}
record_schema! {
    #[short_description = "Write payload; Fence additionally publishes prior channel access completion with release semantics"]
    #[description = "Write payload; Fence additionally publishes prior channel access completion with release semantics."]
    pub record WordWrite {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Destination word"]
            #[description = "Destination word."]
            target: WordTarget,
            #[short_description = "Low selected-width bits to store"]
            #[description = "Low selected-width bits to store."]
            value: u64,
        },
    }
}
record_schema! {
    #[short_description = "Repeated pattern fill with arbitrary byte length"]
    #[description = "Repeated pattern fill with arbitrary byte length."]
    pub record FillCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "Byte-aligned destination address"]
            #[description = "Byte-aligned destination address."]
            address: u64,
            #[short_description = "Byte count including any partial final pattern"]
            #[description = "Byte count including any partial final pattern."]
            length: u64,
            #[short_description = "Repeated low little-endian pattern bytes"]
            #[description = "Repeated low little-endian pattern bytes."]
            pattern: u64,
            #[short_description = "Destination address-space selector"]
            #[description = "Destination address-space selector."]
            context: u32,
            #[short_description = "Pattern width"]
            #[description = "Pattern width."]
            width: enum(u32) { variants: { Byte = 0, Bits32 = 1, Bits64 = 2 } },
        },
    }
}
record_schema! {
    #[short_description = "Retained polling dependency; successful observation has acquire semantics"]
    #[description = "Retained polling dependency; successful observation has acquire semantics."]
    pub record WaitCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "Word loaded for the comparison"]
            #[description = "Word loaded for the comparison."]
            target: WordTarget,
            #[short_description = "Comparison value truncated to selected width"]
            #[description = "Comparison value truncated to selected width."]
            value: u64,
            #[short_description = "Reached uses signed-width modular observed minus value; software distance must remain below half range"]
            #[description = "Reached uses signed-width modular observed minus value; software distance must remain below half range."]
            comparison: enum(u32) { variants: { Equal = 0, NotEqual = 1, UnsignedLess = 2, UnsignedLessEqual = 3, UnsignedGreater = 4, UnsignedGreaterEqual = 5, SignedLess = 6, SignedLessEqual = 7, SignedGreater = 8, SignedGreaterEqual = 9, Reached = 10 } },
        },
    }
}
record_schema! {
    #[short_description = "Atomic semaphore; requires selected-width RMW capability"]
    #[description = "Atomic semaphore; requires selected-width RMW capability."]
    pub record SemaphoreCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "Unsigned permit counter"]
            #[description = "Unsigned permit counter."]
            target: WordTarget,
            #[short_description = "Signal increments with release and rejects overflow before mutation; Wait atomically consumes one positive permit with acquire"]
            #[description = "Signal increments with release and rejects overflow before mutation; Wait atomically consumes one positive permit with acquire."]
            operation: enum(u32) { variants: { Signal = 0, Wait = 1 } },
        },
    }
}
