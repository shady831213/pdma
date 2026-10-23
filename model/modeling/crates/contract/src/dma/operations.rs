mod atomic;
pub use atomic::{
    AtomicCommand, AtomicCommandOperandsAddValues, AtomicCommandOperandsAndValues,
    AtomicCommandOperandsCompareExchangeValues, AtomicCommandOperandsExchangeValues,
    AtomicCommandOperandsOrValues, AtomicCommandOperandsRaw, AtomicCommandOperandsSubtractValues,
    AtomicCommandOperandsValues, AtomicCommandOperandsXorValues, AtomicCommandOperation,
    AtomicCommandOperationRaw, AtomicCommandOrder, AtomicCommandOrderRaw, AtomicCommandRaw,
    AtomicCommandView, AtomicCommandViewMut,
};
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "External timer sample destination"]
    #[description = "Writes one little-endian 64-bit external timer sample. A target PageFault retains the original sample until resume or cancellation; units and timer progression belong to the platform."]
    pub record TimestampCommand {
        alignment: 8, endian: Little,
        fields: {
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
            #[short_description = "Unsigned permit counter"]
            #[description = "Unsigned permit counter."]
            target: WordTarget,
            #[short_description = "Signal increments with release and rejects overflow before mutation; Wait atomically consumes one positive permit with acquire"]
            #[description = "Signal increments with release and rejects overflow before mutation; Wait atomically consumes one positive permit with acquire."]
            operation: enum(u32) { variants: { Signal = 0, Wait = 1 } },
        },
    }
}
