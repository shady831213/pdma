//! Generated PDMA wire-codec behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{Command, CompletionFlagsValues, CopyCommand, Endpoint, Response};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};

#[test]
fn command_round_trips_when_encoded_with_little_endian_padding() -> Result<(), SchemaCodecError> {
    // Given: a command with distinguishable fields.
    let mut command = Command::copy(
        0x0102_0304_0506_0708,
        CopyCommand {
            copy_length: 8,
            sg_context: 0,
            source: Endpoint::linear(0x1112_1314_1516_1718, 0x3132_3334_3536_3738),
            destination: Endpoint::scatter_gather(0x2122_2324_2526_2728, 0x4142_4344_4546_4748),
        },
    );
    command.flags = CompletionFlagsValues { ioc: true };

    // When: the generated codec encodes the complete command.
    let bytes = command.to_bytes()?;

    // Then: scalars are little-endian, alignment padding is zero, and decoding is lossless.
    let mut expected = [0; 112];
    expected[0..8].copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
    expected[24..32].copy_from_slice(&0x1112_1314_1516_1718_u64.to_le_bytes());
    expected[32..40].copy_from_slice(&0x3132_3334_3536_3738_u64.to_le_bytes());
    expected[56] = 1;
    expected[64..72].copy_from_slice(&0x2122_2324_2526_2728_u64.to_le_bytes());
    expected[72..80].copy_from_slice(&0x4142_4344_4546_4748_u64.to_le_bytes());
    expected[96] = 8;
    expected[8] = 1;
    assert_eq!(bytes, expected);
    assert_eq!(Command::from_bytes(&bytes)?, command);
    assert_eq!(Command::SPEC.alignment(), 8);
    Ok(())
}

#[test]
fn response_round_trips_when_status_is_assigned() -> Result<(), SchemaCodecError> {
    let progress = CompletionInfo {
        bytes_completed: 0x0102_0304_0506_0708,
    };
    for (info, discriminant) in [
        (ResponseInfoValues::Success(progress), 0),
        (ResponseInfoValues::InvalidLength(progress), 1),
        (ResponseInfoValues::ReadError(progress), 2),
        (ResponseInfoValues::WriteError(progress), 3),
        (ResponseInfoValues::LengthMismatch(progress), 4),
        (ResponseInfoValues::ListReadError(progress), 5),
    ] {
        // Given: each assigned response status and a visible command ID.
        let response = Response {
            flags: CompletionFlagsValues { ioc: true },
            id: 7,
            status: info.tag(),
            info,
        };

        // When: the generated codec encodes the response.
        let bytes = response.to_bytes()?;

        // Then: the wire status, IOC flag, and decoded value agree.
        let mut expected = [0; 24];
        expected[0] = 7;
        expected[8] = discriminant;
        expected[12] = 1;
        expected[16..24].copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
        assert_eq!(bytes, expected);
        assert_eq!(Response::from_bytes(&bytes)?, response);
        assert_eq!(Response::SPEC.alignment(), 8);
    }
    Ok(())
}

#[test]
fn response_rejects_reserved_status_when_discriminant_is_unassigned() {
    // Given: a response containing the first unassigned status value.
    let mut bytes = [0; Response::RECORD_SIZE];
    bytes[0] = 7;
    bytes[8] = 9;

    // When: raw storage crosses the generated typed boundary.
    let result = Response::from_bytes(&bytes);

    // Then: the error identifies the status field and observed discriminant.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: Response::STATUS_IDENTITY.at_byte(8),
            observed: 9,
        })
    );
}
