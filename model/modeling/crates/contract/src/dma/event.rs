use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "Stopping command progress"]
    #[description = "Stopping command progress."]
    pub record ErrorInfo {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Committed write prefix before the stopping error; never rolled back"]
            #[description = "Committed write prefix before the stopping error; never rolled back."]
            bytes_completed: u64,
        },
    }
}
record_schema! {
    #[short_description = "Error-only event"]
    #[description = "Error-only event. Error stop latches before publication; ring failure escalates to channel fatal."]
    pub record Event {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Original command identifier"]
            #[description = "Original command identifier."]
            id: u64,
            #[short_description = "Stopping error code"]
            #[description = "Stopping error code. Zero is invalid; successful commands produce no event."]
            code: enum(u32) { variants: { InvalidLength = 1, ReadError = 2, WriteError = 3, LengthMismatch = 4, ListReadError = 5, Unsupported = 6, InvalidOperation = 7, ArithmeticOverflow = 8 } },
            #[short_description = "Code-selected error progress"]
            #[description = "Code-selected error progress."]
            info: tagged_union { tag: self.code, cases: { InvalidLength(ErrorInfo), ReadError(ErrorInfo), WriteError(ErrorInfo), LengthMismatch(ErrorInfo), ListReadError(ErrorInfo), Unsupported(ErrorInfo), InvalidOperation(ErrorInfo), ArithmeticOverflow(ErrorInfo) } },
        },
    }
}

impl Event {
    /// Builds a stopping event with progress matching its error code.
    pub const fn stopping(id: u64, code: EventCode, bytes_completed: u64) -> Self {
        let progress = ErrorInfo { bytes_completed };
        let info = match code {
            EventCode::InvalidLength => EventInfoValues::InvalidLength(progress),
            EventCode::ReadError => EventInfoValues::ReadError(progress),
            EventCode::WriteError => EventInfoValues::WriteError(progress),
            EventCode::LengthMismatch => EventInfoValues::LengthMismatch(progress),
            EventCode::ListReadError => EventInfoValues::ListReadError(progress),
            EventCode::Unsupported => EventInfoValues::Unsupported(progress),
            EventCode::InvalidOperation => EventInfoValues::InvalidOperation(progress),
            EventCode::ArithmeticOverflow => EventInfoValues::ArithmeticOverflow(progress),
        };
        Self { id, code, info }
    }

    #[doc = "Committed bytes before the stopping error."]
    pub const fn bytes_completed(&self) -> u64 {
        match self.info {
            EventInfoValues::InvalidLength(info)
            | EventInfoValues::ReadError(info)
            | EventInfoValues::WriteError(info)
            | EventInfoValues::LengthMismatch(info)
            | EventInfoValues::ListReadError(info)
            | EventInfoValues::Unsupported(info)
            | EventInfoValues::InvalidOperation(info)
            | EventInfoValues::ArithmeticOverflow(info) => info.bytes_completed,
        }
    }
}
