use hardware_interface_schema::record_schema;

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
            #[description = "Number of bytes contributed to the endpoint stream. Zero-length segments are skipped. Hardware checks arithmetic only for the selected copy range."]
            length: u64,
        },
    }
}

record_schema! {
    #[short_description = "Endpoint byte position"]
    #[description = "A self-describing continuation position, containing one mode-selected union. Linear has only a base-relative byte offset. SG has an original-table entry index and entry-local byte offset. No state from an earlier hardware command is required."]
    pub record Position {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Position representation"]
            #[description = "0: Linear byte offset. 1: ScatterGather entry index and local offset. Other wire tags are rejected. Encoding derives this tag from the union case."]
            mode: enum(u32) { variants: { Linear = 0, ScatterGather = 1 } },
            #[short_description = "Variant-specific position"]
            #[description = "Linear stores only offset; its unused union word must be zero. SG stores index then offset. At segment end, index advances and offset becomes zero without reading the next entry. Following empty entries may be skipped lazily. A position at count with offset zero denotes exhaustion."]
            payload: union {
                tag: self.mode,
                cases: {
                    Linear { offset: u64 },
                    ScatterGather { index: u64, offset: u64 },
                },
            },
        },
    }
}

impl Position {
    /// A byte offset from a Linear endpoint's base.
    pub const fn linear(offset: u64) -> Self {
        Self {
            mode: PositionMode::Linear,
            payload: PositionPayloadValues::Linear(PositionPayloadLinearValues { offset }),
        }
    }

    /// An entry index and local byte offset in an SG table.
    pub const fn scatter_gather(index: u64, offset: u64) -> Self {
        Self {
            mode: PositionMode::ScatterGather,
            payload: PositionPayloadValues::ScatterGather(PositionPayloadScatterGatherValues {
                index,
                offset,
            }),
        }
    }
}

record_schema! {
    #[short_description = "DMA stream endpoint"]
    #[description = "An independently selected Linear or SG stream with an explicit start position. This 40-byte little-endian record is eight-byte aligned. Lists remain stable while work can use them. Hardware performs no overlap check and imposes no segment-count limit. Copy length is specified once by Command; overlap follows chunk execution order, not whole-transfer memmove semantics."]
    pub record Endpoint {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Endpoint representation"]
            #[description = "0: Linear base address.\n1: ScatterGather table address and total entry count.\nOther wire values are invalid. Encoding derives this tag from the selected payload case."]
            mode: enum(u32) {
                variants: { Linear = 0, ScatterGather = 1 },
            },
            #[short_description = "Payload address-space context"]
            #[description = "Context selector for payload accesses, independent of the context used to read a scatter/gather table. Ignored by ordinary context-free hardware."]
            context: u32,
            #[short_description = "Endpoint parameters"]
            #[description = "Linear supplies a base address. SG supplies the original table address and total count; entries before index are never read. Linear also supplies a byte offset; SG supplies start index and entry-local offset. Only the selected copy range must fit, not the whole table or the opposite endpoint's total capacity. Address and range arithmetic is checked."]
            payload: union {
                tag: self.mode,
                cases: {
                    Linear {
                        address: u64,
                        offset: u64,
                    },
                    ScatterGather {
                        list_address: u64,
                        count: u64,
                        index: u64,
                        offset: u64,
                    },
                },
            },
        },
    }
}

impl Endpoint {
    /// Creates a Linear endpoint at a byte offset from its base.
    pub const fn linear(address: u64, offset: u64) -> Self {
        Self {
            mode: EndpointMode::Linear,
            context: 0,
            payload: EndpointPayloadValues::Linear(EndpointPayloadLinearValues { address, offset }),
        }
    }

    /// Creates an SG endpoint starting at its first entry.
    pub const fn scatter_gather(list_address: u64, count: u64) -> Self {
        Self {
            mode: EndpointMode::ScatterGather,
            context: 0,
            payload: EndpointPayloadValues::ScatterGather(EndpointPayloadScatterGatherValues {
                list_address,
                count,
                index: 0,
                offset: 0,
            }),
        }
    }

    /// Returns the variant-specific starting position.
    pub const fn start(&self) -> Position {
        match self.payload {
            EndpointPayloadValues::Linear(linear) => Position::linear(linear.offset),
            EndpointPayloadValues::ScatterGather(list) => {
                Position::scatter_gather(list.index, list.offset)
            }
        }
    }

    /// Replaces the start without changing backing geometry or context.
    /// Returns None when the position belongs to a different endpoint mode.
    #[must_use]
    pub const fn with_start(mut self, position: Position) -> Option<Self> {
        match (&mut self.payload, position.payload) {
            (EndpointPayloadValues::Linear(linear), PositionPayloadValues::Linear(start)) => {
                linear.offset = start.offset;
            }
            (
                EndpointPayloadValues::ScatterGather(list),
                PositionPayloadValues::ScatterGather(start),
            ) => {
                list.index = start.index;
                list.offset = start.offset;
            }
            (EndpointPayloadValues::Linear(_), PositionPayloadValues::ScatterGather(_))
            | (EndpointPayloadValues::ScatterGather(_), PositionPayloadValues::Linear(_)) => {
                return None;
            }
        }
        self.mode = self.payload.tag();
        Some(self)
    }
}
