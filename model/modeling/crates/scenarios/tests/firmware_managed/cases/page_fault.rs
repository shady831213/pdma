use crate::harness::{TestResult, page_fault};
use pdma_model_contract::{
    dma::{PageFaultKindsAccess, Position, ResponseStatus},
    memory::{ContextAttributes, ContextAttributesAddressSpace},
};

#[tokio::test(flavor = "current_thread")]
async fn page_fault_software_repairs_each_retained_pasid_and_resubmits_without_reset() -> TestResult
{
    // Given: equal payload VAs in independent PASIDs, resident rings/lists and different SG boundaries.
    // When: real firmware provisions contexts and Host software repairs two IRQ-reported missing pages.
    let result = page_fault::run().await?;
    // Then: source repair cannot repair the destination mapping, and only committed bytes count.
    assert_eq!(
        result
            .responses
            .iter()
            .map(|r| (
                r.id,
                r.status,
                r.bytes_completed(),
                r.page_fault().map(|fault| fault.kinds.access)
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                17,
                ResponseStatus::PageFault,
                200,
                Some(PageFaultKindsAccess::SourceRead)
            ),
            (
                18,
                ResponseStatus::PageFault,
                0,
                Some(PageFaultKindsAccess::DestinationWrite)
            ),
            (19, ResponseStatus::Success, 400, None)
        ]
    );
    for response in result.responses.get(..2).ok_or("missing fault responses")? {
        assert_eq!(
            response
                .page_fault()
                .ok_or("expected PageFault")?
                .source_next(),
            Position::scatter_gather(0, 200)
        );
        assert_eq!(
            response
                .page_fault()
                .ok_or("expected PageFault")?
                .destination_next(),
            Position::scatter_gather(1, 0)
        );
        assert_eq!(
            response
                .page_fault()
                .ok_or("expected PageFault")?
                .fault_address,
            0x1100
        );
    }
    assert_eq!(
        result.repairs,
        [1, 2].map(|pasid| (
            ContextAttributes {
                address_space: ContextAttributesAddressSpace::Pasid,
                pasid
            },
            0x1100
        ))
    );
    assert_eq!(
        result.bytes,
        (0..600)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect::<Vec<_>>()
    );
    assert!(result.released);
    Ok(())
}
