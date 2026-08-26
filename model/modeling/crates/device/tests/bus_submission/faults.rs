use super::common::{TestResult, allocate, encode, fixture, pending_event, run_descriptor};
use pdma_model_contract::abi::{
    AbiCommand, AbiEventAccess, AbiEventKind, AbiEventStage, AbiId,
    DescriptorSchemaPayloadCopyValues, DmaAddress, MAX_INDIRECT_DESTINATIONS, RingCursor,
};
use pdma_model_contract::semantic::ResourceConfig;

#[derive(Clone, Copy)]
pub(super) enum BroadcastFaultCase {
    Count,
    Alignment,
    Overflow,
    Unmapped,
    Bounds,
}

impl BroadcastFaultCase {
    pub(super) const ALL: [Self; 5] = [
        Self::Count,
        Self::Alignment,
        Self::Overflow,
        Self::Unmapped,
        Self::Bounds,
    ];
}

pub(super) async fn malformed_and_resource_stall() -> TestResult {
    let malformed = fixture(ResourceConfig::default()).await?;
    let source = allocate(&malformed.handle, 4).await?;
    let destination = allocate(&malformed.handle, 4).await?;
    malformed
        .handle
        .write_memory(source.base(), &[1, 2, 3, 4])
        .await?;
    let mut bytes = encode(AbiCommand::copy(
        AbiId::new(20),
        DescriptorSchemaPayloadCopyValues {
            source_dma_address: source.base().raw(),
            destination_dma_address: destination.base().raw(),
            length: 4,
        },
    ))?;
    bytes[40] = 1;
    let slot = malformed.submission;
    let result = run_descriptor(malformed, bytes, &[destination]).await?;
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(result.destinations, [vec![0; 4]]);
    assert_eq!(result.steps, 1);
    assert_eq!(result.native_steps, 1);
    assert!(!result.irq_pending);
    assert_eq!(
        pending_event(&result)?.fault().tuple(),
        (
            AbiEventKind::InvalidReserved,
            slot.raw(),
            1,
            0,
            AbiEventStage::Descriptor.value(),
            AbiEventAccess::None.value(),
            40,
        )
    );

    let starved = fixture(ResourceConfig::default().with_read_slots(0)).await?;
    let source = allocate(&starved.handle, 4).await?;
    let destination = allocate(&starved.handle, 4).await?;
    starved
        .handle
        .write_memory(source.base(), &[9, 8, 7, 6])
        .await?;
    let result = run_descriptor(
        starved,
        encode(AbiCommand::copy(
            AbiId::new(21),
            DescriptorSchemaPayloadCopyValues {
                source_dma_address: source.base().raw(),
                destination_dma_address: destination.base().raw(),
                length: 4,
            },
        ))?,
        &[destination],
    )
    .await?;
    assert_eq!(result.consumer, RingCursor::new(0));
    assert_eq!(result.pending, None);
    assert_eq!(result.destinations, [vec![0; 4]]);
    assert_eq!(result.steps, 0);
    assert_eq!(result.native_steps, 2);
    assert!(!result.irq_pending);
    Ok(())
}

pub(super) async fn assert_broadcast_fault(case: BroadcastFaultCase) -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let source = allocate(&fixture.handle, 4).await?;
    fixture
        .handle
        .write_memory(source.base(), &[1, 2, 3, 4])
        .await?;
    let list = allocate(&fixture.handle, 8).await?;
    let (list_base, count) = list_fields(case, list);
    let mut bytes = encode(AbiCommand::copy(
        AbiId::new(30),
        DescriptorSchemaPayloadCopyValues {
            source_dma_address: source.base().raw(),
            destination_dma_address: source.base().raw(),
            length: 4,
        },
    ))?;
    bytes[2..4].copy_from_slice(&3_u16.to_le_bytes());
    bytes[24..32].copy_from_slice(&list_base.raw().to_le_bytes());
    bytes[40..48].copy_from_slice(&count.to_le_bytes());
    bytes[48..64].fill(0);
    let result = run_descriptor(fixture, bytes, &[]).await?;
    let expected = expected_tuple(case, list_base, list);
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(pending_event(&result)?.fault().tuple(), expected);
    Ok(())
}

const fn list_fields(
    case: BroadcastFaultCase,
    list: pdma_model_contract::binding::PdmaMemoryRegion,
) -> (DmaAddress, u64) {
    match case {
        BroadcastFaultCase::Count => (list.base(), 0),
        BroadcastFaultCase::Alignment => (DmaAddress::new(list.base().raw() + 1), 1),
        BroadcastFaultCase::Overflow => (DmaAddress::new(u64::MAX - 7), 1),
        BroadcastFaultCase::Unmapped => (DmaAddress::new(8), 1),
        BroadcastFaultCase::Bounds => (list.base(), 2),
    }
}

fn expected_tuple(
    case: BroadcastFaultCase,
    base: DmaAddress,
    list: pdma_model_contract::binding::PdmaMemoryRegion,
) -> (AbiEventKind, u64, u64, u64, u16, u16, u64) {
    let (kind, locator, detail0, detail1, access) = match case {
        BroadcastFaultCase::Count => (
            AbiEventKind::InvalidIndirectCount,
            base.raw(),
            0,
            u64::from(MAX_INDIRECT_DESTINATIONS),
            AbiEventAccess::None,
        ),
        BroadcastFaultCase::Alignment => (
            AbiEventKind::InvalidIndirectAlignment,
            base.raw(),
            1,
            8,
            AbiEventAccess::None,
        ),
        BroadcastFaultCase::Overflow => (
            AbiEventKind::RangeOverflow,
            base.raw(),
            1,
            8,
            AbiEventAccess::Read,
        ),
        BroadcastFaultCase::Unmapped => (
            AbiEventKind::UnmappedDmaAddress,
            base.raw(),
            8,
            0,
            AbiEventAccess::Read,
        ),
        BroadcastFaultCase::Bounds => (
            AbiEventKind::OutOfBounds,
            list.base().raw() + list.length(),
            list.length(),
            list.base().raw() + 16,
            AbiEventAccess::Read,
        ),
    };
    (
        kind,
        locator,
        detail0,
        detail1,
        AbiEventStage::IndirectList.value(),
        access.value(),
        0,
    )
}
