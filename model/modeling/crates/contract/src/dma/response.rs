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
    #[short_description = "Observed signal value"]
    #[description = "The signed 64-bit signal observation, stored as two's-complement bits."]
    pub record SignalInfo {
        endian: Little,
        fields: {
            #[short_description = "Observed value"]
            #[description = "Value observed before an atomic update, or the value satisfying Wait."]
            value: u64,
        },
    }
}

record_schema! {
    #[short_description = "Signal access failure"]
    #[description = "A terminal signal or wait failure which stops the channel."]
    pub record SignalErrorInfo {
        endian: Little,
        fields: {
            #[short_description = "Failure reason"]
            #[description = "InvalidOperation denotes invalid alignment or ordering; Unsupported denotes unavailable atomic access; Access denotes a terminal memory access failure."]
            reason: enum(u32) { variants: { InvalidOperation = 0, Unsupported = 1, Access = 2 } },
        },
    }
}

pub use SignalErrorInfoReason as SignalErrorReason;

record_schema! {
    #[short_description = "DMA response"]
    #[description = "A 24-byte, eight-byte-aligned little-endian terminal completion. Recoverable payload faults suspend the command without publishing a response. Unsuccessful results stop the channel until reset. Ring failures use the channel fatal register. Committed writes are never rolled back."]
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Completed command identifier"]
            #[description = "The identifier copied unchanged from the command, allowing software to associate this response with its submission."]
            id: u64,
            #[short_description = "Command result"]
            #[description = "0: Success. 1: InvalidLength. 2: ReadError. 3: WriteError. 4: LengthMismatch. 5: ListReadError. 7: SignalSuccess. 8: SignalError. Success and SignalSuccess avoid channel halt. Retired PageFault encoding 6 remains invalid."]
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadError = 2,
                    WriteError = 3,
                    LengthMismatch = 4,
                    ListReadError = 5,
                    SignalSuccess = 7,
                    SignalError = 8,
                },
            },
            #[short_description = "Echoed completion flags"]
            #[description = "The command's IOC request, identifying the notification-bearing response without a separate task-pointer register."]
            flags: bits(CompletionFlags),
            #[short_description = "Status-specific completion information"]
            #[description = "Copy statuses report committed bytes; SignalSuccess reports an observed value; SignalError reports a reason."]
            info: union {
                tag: self.status,
                cases: {
                    Success(CompletionInfo),
                    InvalidLength(CompletionInfo),
                    ReadError(CompletionInfo),
                    WriteError(CompletionInfo),
                    LengthMismatch(CompletionInfo),
                    ListReadError(CompletionInfo),
                    SignalSuccess(SignalInfo),
                    SignalError(SignalErrorInfo),
                },
            },
        },
    }
}

impl ResponseStatus {
    /// Whether publication stops the channel until reset.
    pub const fn halts(self) -> bool {
        match self {
            Self::Success | Self::SignalSuccess => false,
            Self::InvalidLength
            | Self::ReadError
            | Self::WriteError
            | Self::LengthMismatch
            | Self::ListReadError
            | Self::SignalError => true,
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
            ResponseInfoValues::SignalSuccess(_) | ResponseInfoValues::SignalError(_) => 0,
        }
    }

    /// Returns a successful signal observation interpreted as a signed value.
    pub const fn signal_value(&self) -> Option<i64> {
        match self.info {
            ResponseInfoValues::SignalSuccess(info) => {
                Some(i64::from_le_bytes(info.value.to_le_bytes()))
            }
            ResponseInfoValues::Success(_)
            | ResponseInfoValues::InvalidLength(_)
            | ResponseInfoValues::ReadError(_)
            | ResponseInfoValues::WriteError(_)
            | ResponseInfoValues::LengthMismatch(_)
            | ResponseInfoValues::ListReadError(_)
            | ResponseInfoValues::SignalError(_) => None,
        }
    }
}
