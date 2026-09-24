use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! {
    pub bits CompletionFlags(u32) {
        #[short_description = "Interrupt on completion"]
        #[description = "Request a channel notification after successful response publication, independently of empty-to-non-empty notification. Echoed in the response; consuming that response clears its IOC cause. Device masks suppress new notifications without replay on unmask."]
        ioc: 0..=0,
    }
}

record_schema! {
    #[short_description = "Scatter/gather segment"]
    #[description = "One contiguous payload span in a software-owned scatter/gather list. The 16-byte little-endian record is eight-byte aligned. Software keeps lists and payload buffers stable until the command completes."]
    pub record Segment {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Payload address"]
            #[description = "First payload byte in the attached memory address space. Zero-length segments perform no payload access and require no usable address."]
            address: u64,
            #[short_description = "Segment byte length"]
            #[description = "Number of bytes contributed to the endpoint stream. Zero-length segments are skipped. Endpoint totals use checked 64-bit arithmetic."]
            length: u64,
        },
    }
}

record_schema! {
    #[short_description = "DMA stream endpoint"]
    #[description = "An independently selected linear or scatter/gather byte stream. The 24-byte little-endian record is eight-byte aligned. List descriptors are read on demand; software keeps descriptors and payload buffers stable until completion. Hardware imposes no segment-count or total-length bound and performs no overlap check. Payload chunks are read then written in logical stream order; overlap follows that deterministic execution order without a whole-transfer memmove guarantee."]
    pub record Endpoint {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Endpoint representation"]
            #[description = "0: Linear, a contiguous address and byte length.\n1: ScatterGather, an address and count of Segment records.\nOther wire values are invalid. Encoding derives this tag from the selected payload case."]
            mode: enum(u32) {
                variants: { Linear = 0, ScatterGather = 1 },
            },
            #[short_description = "Payload address-space context"]
            #[description = "Context selector for payload accesses, independent of the context used to read a scatter/gather table. Ignored by ordinary context-free hardware."]
            context: u32,
            #[short_description = "Endpoint parameters"]
            #[description = "Linear: address is the first payload byte and length is the total byte count; zero length performs no payload access. ScatterGather: list_address points to consecutive eight-byte-aligned Segment records and count gives their number with no hardware limit; zero count performs no list access. List extent and summed lengths use checked arithmetic. The opposite endpoint must have the same total."]
            payload: union {
                tag: self.mode,
                cases: {
                    Linear {
                        address: u64,
                        length: u64,
                    },
                    ScatterGather {
                        list_address: u64,
                        count: u64,
                    },
                },
            },
        },
    }
}

impl Endpoint {
    /// Creates a contiguous endpoint with a matching wire tag.
    pub const fn linear(address: u64, length: u64) -> Self {
        Self {
            mode: EndpointMode::Linear,
            context: 0,
            payload: EndpointPayloadValues::Linear(EndpointPayloadLinearValues { address, length }),
        }
    }

    /// Creates a scatter/gather endpoint with a matching wire tag.
    pub const fn scatter_gather(list_address: u64, count: u64) -> Self {
        Self {
            mode: EndpointMode::ScatterGather,
            context: 0,
            payload: EndpointPayloadValues::ScatterGather(EndpointPayloadScatterGatherValues {
                list_address,
                count,
            }),
        }
    }
}

record_schema! {
    #[short_description = "DMA copy command"]
    #[description = "A software-produced command requesting a copy between two independently described byte streams. The wire record is 64 bytes, eight-byte aligned and little-endian. Hardware checks both endpoint totals before any payload write and reports LengthMismatch if they differ. Software keeps all lists and buffers stable until completion or reset."]
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
            #[description = "Linear or scatter/gather stream receiving payload bytes in logical order. Its total must match the source total."]
            destination: Endpoint,
            #[short_description = "Completion notification flags"]
            #[description = "IOC requests notification after this command's response is published. Reserved bits must be zero."]
            flags: bits(CompletionFlags),
            #[short_description = "Scatter/gather table context"]
            #[description = "Context selector shared by both scatter/gather tables. Entries describe payload in each endpoint's own context. Ignored when both endpoints are linear or hardware is context-free."]
            sg_context: u32,
        },
    }
}

record_schema! {
    #[short_description = "DMA copy response"]
    #[description = "A hardware-produced 24-byte, eight-byte-aligned little-endian response. Any unsuccessful result stops the channel until reset and forces an error notification. Ring-access faults instead use the channel fatal register. Previously committed external memory writes are never rolled back."]
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Completed command identifier"]
            #[description = "The identifier copied unchanged from the command, allowing software to associate this response with its submission."]
            id: u64,
            #[short_description = "Copy result"]
            #[description = "0: Success, including a zero-length copy.\n1: InvalidLength, arithmetic overflow in summed lengths or list extent/address calculations.\n2: ReadFault, payload read failed.\n3: WriteFault, payload write failed.\n4: LengthMismatch, endpoint totals differ before payload writes.\n5: ListReadFault, scatter/gather descriptor read failed.\nOther wire values are invalid. Error responses stop the channel until channel reset without setting the ring-access fatal code."]
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadFault = 2,
                    WriteFault = 3,
                    LengthMismatch = 4,
                    ListReadFault = 5,
                },
            },
            #[short_description = "Echoed completion flags"]
            #[description = "The command's IOC request, identifying the notification-bearing response without a separate task-pointer register."]
            flags: bits(CompletionFlags),
            #[short_description = "Completed payload bytes"]
            #[description = "Bytes successfully written before completion or failure, counted with 64-bit arithmetic. A successful transfer reports its full endpoint total; validation failures before copying report zero."]
            bytes_completed: u64,
        },
    }
}
