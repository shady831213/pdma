use super::endpoint::Endpoint;
use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! {
    pub bits CompletionFlags(u32) {
        #[short_description = "Interrupt on completion"]
        #[description = "Request a channel notification after successful response publication, independently of empty-to-non-empty notification. Echoed in the response; consuming that response clears its IOC cause. Device masks suppress new notifications without replay on unmask."]
        ioc: 0..=0,
    }
}

record_schema! {
    #[short_description = "DMA copy command"]
    #[description = "A 104-byte, eight-byte-aligned little-endian command. Two independent endpoint starts select one copy_length. Hardware validates both selected ranges before payload writes; backing capacities need not match. Software retains lists and resources while any attempt or continuation can use them. Every submission is self-contained."]
    pub record Command {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Command identifier"]
            #[description = "Software-selected identifier echoed unchanged in the response. Hardware does not interpret or enforce uniqueness of this value."]
            id: u64,
            #[short_description = "Source stream"]
            #[description = "Linear or scatter/gather stream supplying payload bytes in logical order."]
            source: Endpoint,
            #[short_description = "Destination stream"]
            #[description = "Independent destination stream receiving copy_length bytes from its selected start."]
            destination: Endpoint,
            #[short_description = "Copy byte length"]
            #[description = "Bytes requested from each endpoint start. Zero performs no list or payload accesses. Each selected SG range must cover this many bytes; insufficient capacity reports LengthMismatch before payload writes."]
            copy_length: u64,
            #[short_description = "Completion notification flags"]
            #[description = "IOC requests notification after this command's response is published. Reserved bits must be zero."]
            flags: bits(CompletionFlags),
            #[short_description = "Scatter/gather table context"]
            #[description = "Context selector shared by both scatter/gather tables. Entries describe payload in each endpoint's own context. Ignored when both endpoints are linear or hardware is context-free."]
            sg_context: u32,
        },
    }
}
