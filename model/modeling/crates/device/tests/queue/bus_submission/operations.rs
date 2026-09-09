use super::common::{Record, TestResult, allocate, encode, fixture, run_descriptor};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues,
};
use pdma_model_contract::semantic::{ResourceConfig, RingCursor};

/// Operation variants covered by the exact descriptor inventory.
#[derive(Clone, Copy, Debug)]
pub enum OperationCase {
    /// Linear copy.
    Copy,
    /// Strided copy.
    Strided,
    /// Broadcast copy.
    Broadcast,
    /// Vector addition.
    Vector,
}

impl OperationCase {
    /// Complete operation inventory.
    pub const ALL: [Self; 4] = [Self::Copy, Self::Strided, Self::Broadcast, Self::Vector];
}

/// Drives one operation through the exact descriptor frontend.
///
/// # Panics
///
/// Panics when the observed exact result violates the pinned operation contract.
pub async fn execute_operation(case: OperationCase) -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let source_a = allocate(&fixture.memory, 8)?;
    let source_b = allocate(&fixture.memory, 8)?;
    let destination_a = allocate(&fixture.memory, 8)?;
    let destination_b = allocate(&fixture.memory, 8)?;
    fixture
        .handle
        .write_memory(source_a.base(), &[1, 2, 3, 4, 5, 6, 7, 8])
        .await?;
    fixture
        .handle
        .write_memory(source_b.base(), &[8, 7, 6, 5, 4, 3, 2, 1])
        .await?;
    let (command, expected_a, expected_b) = command(
        case,
        &fixture,
        source_a,
        source_b,
        destination_a,
        destination_b,
    )
    .await?;
    let descriptor = encode(command)?;
    let result = run_descriptor(fixture, descriptor, &[destination_a, destination_b]).await?;
    assert!(matches!(result.record, Some(Record::Completion(_))));
    assert_eq!(
        result.consumer,
        pdma_model_contract::semantic::RingCursor::new(1)
    );
    assert_eq!(result.destinations, [expected_a, expected_b]);
    assert!(!result.traces.is_empty());
    assert_eq!(result.steps, 1);
    assert_eq!(result.native_steps, 1);
    Ok(())
}

pub(in crate::queue) async fn zero_row_strided() -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let descriptor = encode(AbiCommand::from_payload(
        14,
        AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
            source_dma_address: u64::MAX,
            destination_dma_address: u64::MAX,
            rows: 0.into(),
            row_bytes: u32::MAX.into(),
            source_stride: u32::MAX.into(),
            destination_stride: u32::MAX.into(),
        }),
    ))?;
    let result = run_descriptor(fixture, descriptor, &[]).await?;
    assert!(matches!(result.record, Some(Record::Completion(_))));
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(result.native_steps, 1);
    Ok(())
}

pub(in crate::queue) async fn command(
    case: OperationCase,
    fixture: &super::common::Fixture,
    source_a: crate::queue::hardware::Allocation,
    source_b: crate::queue::hardware::Allocation,
    destination_a: crate::queue::hardware::Allocation,
    destination_b: crate::queue::hardware::Allocation,
) -> TestResult<(AbiCommand, Vec<u8>, Vec<u8>)> {
    match case {
        OperationCase::Copy => Ok((
            AbiCommand::from_payload(
                10,
                AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                    source_dma_address: source_a.base().checked_add(2)?.raw(),
                    destination_dma_address: destination_a.base().checked_add(1)?.raw(),
                    length: 4.into(),
                }),
            ),
            vec![0, 3, 4, 5, 6, 0, 0, 0],
            vec![0; 8],
        )),
        OperationCase::Strided => Ok((
            AbiCommand::from_payload(
                11,
                AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                    source_dma_address: source_a.base().checked_add(1)?.raw(),
                    destination_dma_address: destination_a.base().raw(),
                    rows: 2.into(),
                    row_bytes: 2.into(),
                    source_stride: 3.into(),
                    destination_stride: 3.into(),
                }),
            ),
            vec![2, 3, 0, 5, 6, 0, 0, 0],
            vec![0; 8],
        )),
        OperationCase::Broadcast => {
            let list = allocate(&fixture.memory, 16)?;
            let mut entries = [0_u8; 16];
            entries[0..8].copy_from_slice(&destination_a.base().raw().to_le_bytes());
            entries[8..16].copy_from_slice(&destination_b.base().raw().to_le_bytes());
            fixture.handle.write_memory(list.base(), &entries).await?;
            Ok((
                AbiCommand::from_payload(
                    12,
                    AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
                        source_dma_address: source_a.base().checked_add(2)?.raw(),
                        destination_list_dma_address: list.base().raw(),
                        length: 3.into(),
                        destination_count: 2.into(),
                    }),
                ),
                vec![3, 4, 5, 0, 0, 0, 0, 0],
                vec![3, 4, 5, 0, 0, 0, 0, 0],
            ))
        }
        OperationCase::Vector => Ok((
            AbiCommand::from_payload(
                13,
                AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                    source_a_dma_address: source_a.base().checked_add(1)?.raw(),
                    source_b_dma_address: source_b.base().checked_add(1)?.raw(),
                    destination_dma_address: destination_a.base().checked_add(2)?.raw(),
                    length: 4.into(),
                }),
            ),
            vec![0, 0, 9, 9, 9, 9, 0, 0],
            vec![0; 8],
        )),
    }
}
