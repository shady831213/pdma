//! IOC bits are shared wire metadata, not a separate command protocol.

use pdma_model_contract::dma::{
    Command, CompletionFlagsValues, Endpoint, Response, ResponseStatus,
};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};

#[test]
fn ioc_round_trips_in_command_and_response() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let flags = CompletionFlagsValues { ioc: true };
    let command = Command {
        copy_length: 4,
        sg_context: 0,
        id: 7,
        source: Endpoint::linear(0x100, 0),
        destination: Endpoint::linear(0x200, 0),
        flags,
    };
    let response = Response {
        id: 7,
        status: ResponseStatus::Success,
        flags,
        info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 4 }),
    };
    // When
    let command_bytes = command.to_bytes()?;
    let response_bytes = response.to_bytes()?;
    // Then
    assert_eq!(command_bytes[96], 1);
    assert_eq!(response_bytes[12], 1);
    assert_eq!(Command::from_bytes(&command_bytes)?, command);
    assert_eq!(Response::from_bytes(&response_bytes)?, response);
    Ok(())
}
