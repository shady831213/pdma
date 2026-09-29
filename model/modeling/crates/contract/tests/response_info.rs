//! Status-selected completion information and compact fault layout.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{
    PageFaultInfo, PageFaultKindsAccess, Position, Response, ResponseInfoValues, ResponseStatus,
};

#[test]
fn fault_info_preserves_all_position_pairs_in_compact_storage()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: every source/destination mode combination with full-width positions.
    let positions = [
        Position::linear(u64::MAX),
        Position::scatter_gather(u64::MAX - 1, u64::MAX),
    ];
    for source in positions {
        for destination in positions {
            let fault = PageFaultInfo::from_positions(
                17,
                0x1234,
                PageFaultKindsAccess::DestinationWrite,
                source,
                destination,
            );
            let response = Response {
                info: ResponseInfoValues::PageFault(fault),
                ..Response::ZERO
            };
            // When: the status-selected union is encoded and decoded.
            let bytes = response.to_bytes()?;
            let decoded = Response::from_bytes(&bytes)?;
            // Then: union cases determine status and positions without redundant tag padding.
            assert_eq!(Response::RECORD_SIZE, 72);
            assert_eq!(Response::INFO_OFFSET, 16);
            assert_eq!(PageFaultInfo::RECORD_SIZE, 56);
            assert_eq!(
                [bytes[8], bytes[16], bytes[24], bytes[25], bytes[32]],
                [6, 17, 0x34, 0x12, 2]
            );
            assert_eq!(&bytes[35..40], &[0; 5]);
            assert_eq!(decoded.status, ResponseStatus::PageFault);
            let decoded = decoded.page_fault().ok_or("missing fault info")?;
            assert_eq!(decoded.source_next(), source);
            assert_eq!(decoded.destination_next(), destination);
        }
    }
    Ok(())
}

#[test]
fn success_and_error_cannot_carry_inactive_fault_information() {
    // Given: every non-fault status with a nonzero fault-only byte.
    for status in 0..=5 {
        let mut bytes = [0; Response::RECORD_SIZE];
        bytes[8] = status;
        bytes[24] = 1;
        // When / Then: the generated union rejects data outside that status's record.
        assert!(matches!(
            Response::from_bytes(&bytes),
            Err(SchemaCodecError::NonZeroReserved { observed: 1, .. })
        ));
        bytes[24] = 0;
        assert!(Response::from_bytes(&bytes).is_ok_and(|response| response.page_fault().is_none()));
    }
}
