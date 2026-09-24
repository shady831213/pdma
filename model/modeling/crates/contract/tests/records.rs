//! Generated PDMA wire-codec behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{
    Command, CompletionFlagsValues, Endpoint, Response, ResponseStatus,
};

#[test]
fn command_round_trips_when_encoded_with_little_endian_padding() -> Result<(), SchemaCodecError> {
    // Given: a command with distinguishable fields.
    let command = Command {
        sg_context: 0,
        flags: CompletionFlagsValues { ioc: true },
        id: 0x0102_0304_0506_0708,
        source: Endpoint::linear(0x1112_1314_1516_1718, 0x3132_3334_3536_3738),
        destination: Endpoint::scatter_gather(0x2122_2324_2526_2728, 0x4142_4344_4546_4748),
    };

    // When: the generated codec encodes the complete command.
    let bytes = command.to_bytes()?;

    // Then: scalars are little-endian, alignment padding is zero, and decoding is lossless.
    assert_eq!(
        bytes,
        [
            8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 1, 0, 0, 0, 0, 0, 0, 0,
            0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43,
            0x42, 0x41, 1, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(Command::from_bytes(&bytes)?, command);
    assert_eq!(Command::SPEC.alignment(), 8);
    Ok(())
}

#[test]
fn response_round_trips_when_status_is_assigned() -> Result<(), SchemaCodecError> {
    for (status, discriminant) in [
        (ResponseStatus::Success, 0),
        (ResponseStatus::InvalidLength, 1),
        (ResponseStatus::ReadFault, 2),
        (ResponseStatus::WriteFault, 3),
        (ResponseStatus::LengthMismatch, 4),
        (ResponseStatus::ListReadFault, 5),
    ] {
        // Given: each assigned response status and a visible command ID.
        let response = Response {
            flags: CompletionFlagsValues { ioc: true },
            id: 7,
            status,
            bytes_completed: 0x0102_0304_0506_0708,
        };

        // When: the generated codec encodes the response.
        let bytes = response.to_bytes()?;

        // Then: the wire status, IOC flag, and decoded value agree.
        assert_eq!(
            bytes,
            [
                7,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                discriminant,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                8,
                7,
                6,
                5,
                4,
                3,
                2,
                1
            ]
        );
        assert_eq!(Response::from_bytes(&bytes)?, response);
        assert_eq!(Response::SPEC.alignment(), 8);
    }
    Ok(())
}

#[test]
fn response_rejects_reserved_status_when_discriminant_is_unassigned() {
    // Given: a response containing the first unassigned status value.
    let bytes = [
        7, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    // When: raw storage crosses the generated typed boundary.
    let result = Response::from_bytes(&bytes);

    // Then: the error identifies the status field and observed discriminant.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: Response::STATUS_IDENTITY.at_byte(8),
            observed: 6,
        })
    );
}
