use super::command::CompletionFlags;
use super::endpoint::{Position, PositionPayloadValues};
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Completed copy progress"]
    #[description = "Success or stopping-error progress, without page-fault-only fields."]
    pub record CompletionInfo {
        endian: Little,
        fields: {
            #[short_description = "Committed prefix"]
            #[description = "Bytes successfully written by this attempt; source reads alone do not count. Success reports copy_length and preflight errors report zero."]
            bytes_completed: u64,
        },
    }
}

record_schema! {
    #[short_description = "DMA copy response"]
    #[description = "A 72-byte, eight-byte-aligned little-endian completion. PageFault terminates one attempt without halting the channel, using ordinary non-empty/IOC notification. Other unsuccessful results stop the channel until reset. Ring failures use the channel fatal register. Committed writes are never rolled back."]
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Completed command identifier"]
            #[description = "The identifier copied unchanged from the command, allowing software to associate this response with its submission."]
            id: u64,
            #[short_description = "Copy result"]
            #[description = "0: Success. 1: InvalidLength, invalid position or overflowing range. 2: ReadError. 3: WriteError. 4: LengthMismatch, selected SG capacity is insufficient. 5: ListReadError. 6: PageFault, a payload mapping is not present and software may repair/resubmit. Only Success and PageFault avoid channel halt. Other values are invalid."]
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadError = 2,
                    WriteError = 3,
                    LengthMismatch = 4,
                    ListReadError = 5,
                    PageFault = 6,
                },
            },
            #[short_description = "Echoed completion flags"]
            #[description = "The command's IOC request, identifying the notification-bearing response without a separate task-pointer register."]
            flags: bits(CompletionFlags),
            #[short_description = "Status-specific completion information"]
            #[description = "The status selects the only valid information format. Success and stopping errors report committed bytes; PageFault additionally reports access and continuation data. Unused case storage must be zero."]
            info: union {
                tag: self.status,
                cases: {
                    Success(CompletionInfo),
                    InvalidLength(CompletionInfo),
                    ReadError(CompletionInfo),
                    WriteError(CompletionInfo),
                    LengthMismatch(CompletionInfo),
                    ListReadError(CompletionInfo),
                    PageFault(PageFaultInfo),
                },
            },
        },
    }
}

impl ResponseStatus {
    /// Whether publication stops the channel until reset.
    pub const fn halts(self) -> bool {
        match self {
            Self::Success | Self::PageFault => false,
            Self::InvalidLength
            | Self::ReadError
            | Self::WriteError
            | Self::LengthMismatch
            | Self::ListReadError => true,
        }
    }
}

impl Response {
    /// Committed bytes for any terminal status.
    pub const fn bytes_completed(&self) -> u64 {
        match self.info {
            ResponseInfoValues::Success(info)
            | ResponseInfoValues::InvalidLength(info)
            | ResponseInfoValues::ReadError(info)
            | ResponseInfoValues::WriteError(info)
            | ResponseInfoValues::LengthMismatch(info)
            | ResponseInfoValues::ListReadError(info) => info.bytes_completed,
            ResponseInfoValues::PageFault(info) => info.bytes_completed,
        }
    }

    /// Page-fault information exists only for the `PageFault` case.
    pub const fn page_fault(&self) -> Option<PageFaultInfo> {
        match self.info {
            ResponseInfoValues::PageFault(info) => Some(info),
            _ => None,
        }
    }
}

record_schema! {
    #[short_description = "Page-fault discriminators"]
    #[description = "Three adjacent bytes identify the failed access and independent source/destination position representations."]
    pub record PageFaultKinds {
        endian: Little,
        fields: {
            #[short_description = "Failed payload access"]
            #[description = "SourceRead requires a readable mapping, DestinationWrite requires a writable mapping. Software checks authorization before repair."]
            access: enum(u8) { variants: { SourceRead = 1, DestinationWrite = 2 } },
            #[short_description = "Source position mode"]
            #[description = "Selected by the source position union, not separately configured."]
            source_mode: enum(u8) { variants: { Linear = 0, ScatterGather = 1 } },
            #[short_description = "Destination position mode"]
            #[description = "Selected independently by the destination position union."]
            destination_mode: enum(u8) { variants: { Linear = 0, ScatterGather = 1 } },
        },
    }
}

record_schema! {
    #[short_description = "Repairable payload page-fault information"]
    #[description = "Committed progress and two independent first-uncommitted positions. The 56-byte record groups discriminator bytes to avoid per-position tag padding. No context is retained by hardware after retirement."]
    pub record PageFaultInfo {
        endian: Little,
        fields: {
            #[short_description = "Committed prefix"]
            #[description = "Successfully written bytes relative to this attempt. Software subtracts this from its submitted copy_length."]
            bytes_completed: u64,
            #[short_description = "Faulting virtual address"]
            #[description = "First missing virtual byte of the failed payload access, not a PTE address. The command ID and retained context association identify its address space."]
            fault_address: u64,
            #[short_description = "Access and position discriminators"]
            #[description = "Grouped small fields; the generated codec derives position modes from their union cases."]
            kinds: PageFaultKinds,
            #[short_description = "Source continuation"]
            #[description = "Linear offset or original SG index/local offset for the first uncommitted source byte. Unused union storage must be zero."]
            source: union { tag: self.kinds.source_mode, cases: {
                Linear { offset: u64 }, ScatterGather { index: u64, offset: u64 },
            } },
            #[short_description = "Destination continuation"]
            #[description = "Independent destination position at the same committed prefix. At segment end, advance index and set offset zero without fetching another entry."]
            destination: union { tag: self.kinds.destination_mode, cases: {
                Linear { offset: u64 }, ScatterGather { index: u64, offset: u64 },
            } },
        },
    }
}

impl PageFaultInfo {
    /// Builds compact fault information from the two typed stream positions.
    pub const fn from_positions(
        bytes_completed: u64,
        fault_address: u64,
        access: PageFaultKindsAccess,
        source: Position,
        destination: Position,
    ) -> Self {
        let source = match source.payload {
            PositionPayloadValues::Linear(value) => {
                PageFaultInfoSourceValues::Linear(PageFaultInfoSourceLinearValues {
                    offset: value.offset,
                })
            }
            PositionPayloadValues::ScatterGather(value) => {
                PageFaultInfoSourceValues::ScatterGather(PageFaultInfoSourceScatterGatherValues {
                    index: value.index,
                    offset: value.offset,
                })
            }
        };
        let destination = match destination.payload {
            PositionPayloadValues::Linear(value) => {
                PageFaultInfoDestinationValues::Linear(PageFaultInfoDestinationLinearValues {
                    offset: value.offset,
                })
            }
            PositionPayloadValues::ScatterGather(value) => {
                PageFaultInfoDestinationValues::ScatterGather(
                    PageFaultInfoDestinationScatterGatherValues {
                        index: value.index,
                        offset: value.offset,
                    },
                )
            }
        };
        Self {
            bytes_completed,
            fault_address,
            kinds: PageFaultKinds {
                access,
                source_mode: source.tag(),
                destination_mode: destination.tag(),
            },
            source,
            destination,
        }
    }

    /// First uncommitted source position, suitable for `Endpoint::with_start`.
    pub const fn source_next(self) -> Position {
        match self.source {
            PageFaultInfoSourceValues::Linear(value) => Position::linear(value.offset),
            PageFaultInfoSourceValues::ScatterGather(value) => {
                Position::scatter_gather(value.index, value.offset)
            }
        }
    }

    /// First uncommitted destination position, independent of the source layout.
    pub const fn destination_next(self) -> Position {
        match self.destination {
            PageFaultInfoDestinationValues::Linear(value) => Position::linear(value.offset),
            PageFaultInfoDestinationValues::ScatterGather(value) => {
                Position::scatter_gather(value.index, value.offset)
            }
        }
    }
}
