//! Terminal response codec boundaries.

use pdma_model_contract::dma::{CompletionInfo, Response, ResponseInfoValues};

#[test]
fn terminal_info_roundtrips_without_retired_fault_storage() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: every terminal result with a nontrivial committed byte count.
    let progress = CompletionInfo {
        bytes_completed: 0x0102_0304_0506_0708,
    };
    for info in [
        ResponseInfoValues::Success(progress),
        ResponseInfoValues::InvalidLength(progress),
        ResponseInfoValues::ReadError(progress),
        ResponseInfoValues::WriteError(progress),
        ResponseInfoValues::LengthMismatch(progress),
        ResponseInfoValues::ListReadError(progress),
    ] {
        let response = Response {
            id: 91,
            status: info.tag(),
            info,
            ..Response::ZERO
        };
        // When: software decodes one terminal response from the wire.
        let bytes = response.to_bytes()?;
        let decoded = Response::from_bytes(&bytes)?;
        // Then: only original identity and terminal progress cross the boundary.
        assert_eq!(bytes.len(), 24);
        assert_eq!(&bytes[16..24], &progress.bytes_completed.to_le_bytes());
        assert_eq!(decoded, response);
    }
    Ok(())
}
