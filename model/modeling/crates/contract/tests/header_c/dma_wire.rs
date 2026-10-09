use pdma_model_contract::dma::ResponseInfoValues;
use pdma_model_contract::dma::{
    Command, CompletionFlagsValues, Endpoint, Position, Response, ResponseStatus, Segment,
};

/// Encodes the Rust fixtures and verifies them against independent wire bytes.
///
/// # Panics
/// Panics if the generated Rust encoding differs from the wire fixtures.
pub fn bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut command_bytes = [0; 104];
    for (index, value) in [
        (0, 7),
        (12, 11),
        (17, 1),
        (24, 7),
        (48, 1),
        (52, 22),
        (57, 2),
        (64, 2),
        (72, 1),
        (80, 3),
        (88, 8),
        (96, 1),
        (100, 33),
    ] {
        *command_bytes
            .get_mut(index)
            .ok_or("command fixture offset")? = value;
    }
    let mut response_bytes = [0; 24];
    for (index, value) in [(0, 7), (8, 3), (12, 1), (16, 8)] {
        *response_bytes
            .get_mut(index)
            .ok_or("response fixture offset")? = value;
    }
    let segment_bytes = [0, 3, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0];
    let command = Command {
        copy_length: 8,
        sg_context: 33,
        flags: CompletionFlagsValues { ioc: true },
        id: 7,
        source: Endpoint {
            context: 11,
            ..Endpoint::linear(0x100, 7)
        },
        destination: Endpoint {
            context: 22,
            ..Endpoint::scatter_gather(0x200, 2)
                .with_start(Position::scatter_gather(1, 3))
                .ok_or("SG position")?
        },
    };
    let response = Response {
        flags: CompletionFlagsValues { ioc: true },
        id: 7,
        status: ResponseStatus::WriteError,
        info: ResponseInfoValues::WriteError(pdma_model_contract::dma::CompletionInfo {
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
