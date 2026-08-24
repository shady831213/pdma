use super::common::{TestResult, allocate, encode, fixture, pending_event, run_descriptor};
use pdma_model_contract::abi::{
    AbiCommand, AbiCopy, AbiEventAccess, AbiEventKind, AbiEventStage, AbiId, DmaAddress,
    MAX_TRANSFER_BYTES,
};
use pdma_model_contract::abi::{CopyDmaAddresses, RingCursor};
use pdma_model_contract::semantic::ResourceConfig;

#[derive(Clone, Copy)]
pub(super) enum HeaderFaultCase {
    Version,
    Size,
    Opcode,
    Flags,
    Reserved,
    Scalar,
}

impl HeaderFaultCase {
    pub(super) const ALL: [Self; 6] = [
        Self::Version,
        Self::Size,
        Self::Opcode,
        Self::Flags,
        Self::Reserved,
        Self::Scalar,
    ];
}

#[derive(Clone, Copy)]
pub(super) enum DirectFaultCase {
    Overflow,
    Unmapped,
    Bounds,
}

impl DirectFaultCase {
    pub(super) const ALL: [Self; 3] = [Self::Overflow, Self::Unmapped, Self::Bounds];
}

pub(super) async fn assert_header_fault(case: HeaderFaultCase) -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let source = allocate(&fixture.handle, 4).await?;
    let destination = allocate(&fixture.handle, 4).await?;
    let mut bytes = encode(AbiCommand::Copy(AbiCopy::new(
        AbiId::new(31),
        CopyDmaAddresses::new(source.base(), destination.base()),
        4,
    )))?;
    let (kind, index, observed, expected) = mutate_header(case, &mut bytes);
    let slot = fixture.submission;
    let result = run_descriptor(fixture, bytes, &[destination]).await?;
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(result.destinations, [vec![0; 4]]);
    assert_eq!(
        pending_event(&result)?.fault().tuple(),
        (
            kind,
            slot.raw(),
            observed,
            expected,
            AbiEventStage::Descriptor.value(),
            AbiEventAccess::None.value(),
            index,
        )
    );
    assert_eq!(result.steps, 1);
    assert_eq!(result.native_steps, 1);
    assert!(!result.irq_pending);
    Ok(())
}

fn mutate_header(case: HeaderFaultCase, bytes: &mut [u8; 64]) -> (AbiEventKind, u64, u64, u64) {
    match case {
        HeaderFaultCase::Version => {
            bytes[0..2].copy_from_slice(&2_u16.to_le_bytes());
            bytes[6..8].copy_from_slice(&63_u16.to_le_bytes());
            (AbiEventKind::InvalidVersion, 0, 2, 1)
        }
        HeaderFaultCase::Size => {
            bytes[6..8].copy_from_slice(&63_u16.to_le_bytes());
            bytes[2..4].copy_from_slice(&5_u16.to_le_bytes());
            (AbiEventKind::InvalidSize, 6, 63, 64)
        }
        HeaderFaultCase::Opcode => {
            bytes[2..4].copy_from_slice(&5_u16.to_le_bytes());
            bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
            (AbiEventKind::InvalidOpcode, 2, 5, 4)
        }
        HeaderFaultCase::Flags => {
            bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
            bytes[40] = 1;
            (AbiEventKind::InvalidFlags, 4, 1, 0)
        }
        HeaderFaultCase::Reserved => {
            bytes[40] = 1;
            bytes[48] = 2;
            (AbiEventKind::InvalidReserved, 40, 1, 0)
        }
        HeaderFaultCase::Scalar => {
            bytes[32..40].copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
            bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
            (
                AbiEventKind::ValueOutOfRange,
                32,
                u64::from(u32::MAX) + 1,
                u64::from(MAX_TRANSFER_BYTES),
            )
        }
    }
}

pub(super) async fn assert_direct_fault(case: DirectFaultCase) -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let mapped = allocate(&fixture.handle, 4).await?;
    let destination = allocate(&fixture.handle, 4).await?;
    let source = match case {
        DirectFaultCase::Overflow => DmaAddress::new(u64::MAX - 1),
        DirectFaultCase::Unmapped => DmaAddress::new(8),
        DirectFaultCase::Bounds => mapped.base().checked_add(2)?,
    };
    let result = run_descriptor(
        fixture,
        encode(AbiCommand::Copy(AbiCopy::new(
            AbiId::new(32),
            CopyDmaAddresses::new(source, DmaAddress::new(u64::MAX)),
            4,
        )))?,
        &[destination],
    )
    .await?;
    let expected = match case {
        DirectFaultCase::Overflow => (AbiEventKind::RangeOverflow, source.raw(), 4, 0),
        DirectFaultCase::Unmapped => (AbiEventKind::UnmappedDmaAddress, source.raw(), 4, 0),
        DirectFaultCase::Bounds => (
            AbiEventKind::OutOfBounds,
            mapped.base().raw() + mapped.length(),
            mapped.length(),
            source.raw() + 4,
        ),
    };
    assert_eq!(
        pending_event(&result)?.fault().tuple(),
        (
            expected.0,
            expected.1,
            expected.2,
            expected.3,
            AbiEventStage::Source.value(),
            AbiEventAccess::Read.value(),
            0,
        )
    );
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(result.destinations, [vec![0; 4]]);
    assert_eq!(result.steps, 1);
    assert_eq!(result.native_steps, 1);
    assert!(!result.irq_pending);
    Ok(())
}
