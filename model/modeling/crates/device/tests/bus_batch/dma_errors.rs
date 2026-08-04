use pdma_model_contract::BufferId;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
use pdma_model_contract::abi::{DmaAddress, DmaRegion};
use pdma_model_device::{DmaBatchContext, DmaSpaceError, MmioBatchError};

#[test]
fn all_dma_errors_map_exhaustively_with_attempted_context() {
    // Given: independent attempted access and mapping coordinates.
    let access = DmaBatchContext::Access {
        address: DmaAddress::new(0x1234),
        length: 0x40,
    };
    let region = DmaRegion::try_new(BufferId::new(9), DmaAddress::new(0x2000), 0x80)
        .expect("valid context region");
    let cases = [
        (
            DmaSpaceError::AddressExhausted,
            DmaBatchContext::None,
            MmioResult::DmaAddressExhausted,
            [0, 0, 0],
        ),
        (
            DmaSpaceError::StorageExhausted,
            DmaBatchContext::None,
            MmioResult::DmaStorageExhausted,
            [0, 0, 0],
        ),
        (
            DmaSpaceError::OverlappingRegion,
            DmaBatchContext::Region(region),
            MmioResult::DmaOverlap,
            [0x2000, 0x80, 0],
        ),
        (
            DmaSpaceError::InvalidRegion,
            DmaBatchContext::Region(region),
            MmioResult::DmaInvalidRegion,
            [0x2000, 0x80, 0],
        ),
        (
            DmaSpaceError::StaleStage {
                expected_generation: 4,
                actual_generation: 5,
            },
            DmaBatchContext::None,
            MmioResult::DmaStaleStage,
            [4, 5, 0],
        ),
        (
            DmaSpaceError::Unmapped,
            access,
            MmioResult::DmaUnmapped,
            [0x1234, 0x40, 0],
        ),
        (
            DmaSpaceError::CrossesRegion,
            access,
            MmioResult::DmaCrossesRegion,
            [0x1234, 0x40, 0],
        ),
    ];

    // When/Then: each source maps to its assigned result and exact three details.
    for (source, context, result, details) in cases {
        assert_eq!(
            MmioBatchError::from_dma(source, context),
            MmioBatchError::Dma(MmioResultDetails { result, details })
        );
    }
    println!("{{\"dma_error_variants\":7,\"access\":[4660,64,0],\"region\":[8192,128,0]}}");
}
