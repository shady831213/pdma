#![doc = "Error event codec boundaries."]

use pdma_model_contract::dma::{ErrorInfo, Event, EventCode, EventInfoValues};

#[test]
fn constructed_events_roundtrip_when_each_stopping_code_is_selected()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: every stopping code with distinct command identity and progress.
    for (index, code) in [
        EventCode::InvalidLength,
        EventCode::ReadError,
        EventCode::WriteError,
        EventCode::LengthMismatch,
        EventCode::ListReadError,
        EventCode::Unsupported,
        EventCode::InvalidOperation,
        EventCode::ArithmeticOverflow,
    ]
    .into_iter()
    .enumerate()
    {
        let index = u64::try_from(index)?;
        let id = 0x1234_5678_9abc_de00 | index;
        let bytes_completed = 0x0102_0304_0506_0700 | index;
        // When: the constructed event crosses the real wire codec.
        let bytes = Event::stopping(id, code, bytes_completed).to_bytes()?;
        let decoded = Event::from_bytes(&bytes)?;
        // Then: identity, selected code and committed progress survive together.
        assert_eq!(decoded.id, id);
        assert_eq!(decoded.code, code);
        assert_eq!(decoded.info.tag(), code);
        assert_eq!(decoded.bytes_completed(), bytes_completed);
    }
    Ok(())
}

#[test]
fn terminal_info_roundtrips_without_retired_fault_storage() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: every terminal result with a nontrivial committed byte count.
    let progress = ErrorInfo {
        bytes_completed: 0x0102_0304_0506_0708,
    };
    for info in [
        EventInfoValues::InvalidLength(progress),
        EventInfoValues::ReadError(progress),
        EventInfoValues::WriteError(progress),
        EventInfoValues::LengthMismatch(progress),
        EventInfoValues::ListReadError(progress),
        EventInfoValues::Unsupported(progress),
        EventInfoValues::InvalidOperation(progress),
        EventInfoValues::ArithmeticOverflow(progress),
    ] {
        let event = Event {
            id: 91,
            code: info.tag(),
            info,
        };
        // When: software decodes one terminal event from the wire.
        let bytes = event.to_bytes()?;
        let decoded = Event::from_bytes(&bytes)?;
        // Then: only original identity and terminal progress cross the boundary.
        assert_eq!(bytes.len(), 24);
        assert_eq!(&bytes[16..24], &progress.bytes_completed.to_le_bytes());
        assert_eq!(decoded, event);
    }
    Ok(())
}
