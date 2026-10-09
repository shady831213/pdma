use super::command::CompletionFlags;
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Completed copy progress"]
    #[description = "Final successful or stopping-error progress of the original command."]
    pub record CompletionInfo {
        endian: Little,
        fields: {
            #[short_description = "Committed prefix"]
            #[description = "Bytes successfully written by this command; source reads alone do not count. Success reports copy_length and preflight errors report zero."]
            bytes_completed: u64,
        },
    }
}

record_schema! {
    #[short_description = "DMA copy response"]
    #[description = "A 24-byte, eight-byte-aligned little-endian terminal completion. Recoverable payload faults suspend the command without publishing a response. Unsuccessful results stop the channel until reset. Ring failures use the channel fatal register. Committed writes are never rolled back."]
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Completed command identifier"]
            #[description = "The identifier copied unchanged from the command, allowing software to associate this response with its submission."]
            id: u64,
            #[short_description = "Copy result"]
            #[description = "0: Success. 1: InvalidLength, invalid position or overflowing range. 2: ReadError. 3: WriteError. 4: LengthMismatch, selected SG capacity is insufficient. 5: ListReadError. Only Success avoids channel halt. Other values, including the retired PageFault encoding 6, are invalid."]
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadError = 2,
                    WriteError = 3,
                    LengthMismatch = 4,
                    ListReadError = 5,
                },
            },
            #[short_description = "Echoed completion flags"]
            #[description = "The command's IOC request, identifying the notification-bearing response without a separate task-pointer register."]
            flags: bits(CompletionFlags),
            #[short_description = "Status-specific completion information"]
            #[description = "The status selects the only valid information format. Success and stopping errors report committed bytes."]
            info: union {
                tag: self.status,
                cases: {
                    Success(CompletionInfo),
                    InvalidLength(CompletionInfo),
                    ReadError(CompletionInfo),
                    WriteError(CompletionInfo),
                    LengthMismatch(CompletionInfo),
                    ListReadError(CompletionInfo),
                },
            },
        },
    }
}

impl ResponseStatus {
    /// Whether publication stops the channel until reset.
    pub const fn halts(self) -> bool {
        match self {
            Self::Success => false,
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
        }
    }
}
