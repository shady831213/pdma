use pdma_model_contract::dma::{
    CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint, ScatterGatherEndpoint,
};

use pdma_model_contract::dma::EventInfoValues;
use pdma_model_contract::dma::{Command, CopyCommand, Event, EventCode, Segment};
use pdma_model_contract::dma::{CommandHeader, Opcode};

/// Encodes the Rust fixtures and verifies them against independent wire bytes.
///
/// # Panics
/// Panics if the generated Rust encoding differs from the wire fixtures.
pub fn bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut command_bytes = [0; 96];
    for (index, value) in [
        (8, 7),
        (1, 2),
        (4, 33),
        (16, 8),
        (24, 11),
        (28, 22),
        (33, 1),
        (40, 7),
        (65, 2),
        (72, 2),
        (80, 1),
        (88, 3),
    ] {
        *command_bytes
            .get_mut(index)
            .ok_or("command fixture offset")? = value;
    }
    let mut response_bytes = [0; 24];
    for (index, value) in [(0, 7), (8, 3), (16, 8)] {
        *response_bytes
            .get_mut(index)
            .ok_or("response fixture offset")? = value;
    }
    let segment_bytes = [0, 3, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0];
    let command = Command::Copy(CopyCommand {
        header: CommandHeader {
            opcode: Opcode::Copy,
        },
        id: 7,
        copy_length: 8,
        sg_context: 33,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0x100,
            offset: 7,
        }),
        source_context: 11,
        destination: CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0x200,
            count: 2,
            index: 1,
            offset: 3,
        }),
        destination_context: 22,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    let response = Event {
        id: 7,
        code: EventCode::WriteError,
        info: EventInfoValues::WriteError(pdma_model_contract::dma::ErrorInfo {
            bytes_completed: 8,
        }),
    };
    let segment = Segment {
        address: 0x300,
        length: 8,
    };
    assert_eq!(command.to_bytes()?, command_bytes);
    assert_eq!(response.to_bytes()?, response_bytes);
    assert_eq!(segment.to_bytes()?, segment_bytes);
    Ok([
        command_bytes.as_slice(),
        response_bytes.as_slice(),
        segment_bytes.as_slice(),
    ]
    .concat())
}
