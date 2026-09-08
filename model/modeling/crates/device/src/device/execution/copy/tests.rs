use super::{CommandRoute, ExecutionOutcome, execute_strided};
use crate::adapter::command::decode;
use crate::operation::DecodedOperation;
use crate::{PdmaMemoryMappings, PdmaMemoryProvisioning};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues,
};
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};

#[test]
fn source_extent_fault_precedes_unmapped_destination_start() {
    // Given: a mapped source with an oversized extent and an unmapped destination.
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut memory = PdmaMemoryMappings::new(provisioning.memory_access());
    let source = provisioning.allocate(&mut memory, 4).expect("source");
    let request = crate::operation::CopyOperands {
        source: source.base(),
        destination: pdma_model_contract::abi::DmaAddress::new(0),
        length: 8,
    };
    let route = CommandRoute::new(
        QueueId::new(0),
        CommandId::new(17),
        CompletionRingId::new(0),
        true,
    );
    // When: the data operation resolves its operands in execution order.
    let outcome = super::execute(&memory, route, request);
    // Then: the source bounds fault wins before destination mapping is consulted.
    let ExecutionOutcome::Fault(fault) = outcome else {
        panic!("source fault");
    };
    assert_eq!(
        fault.error_kind,
        pdma_model_contract::semantic::EventErrorKind::OutOfBounds { len: 4, end: 8 }
    );
    assert_eq!(
        fault.exact.tuple(),
        (
            pdma_model_contract::abi::AbiEventKind::OutOfBounds,
            source.base().raw() + 4,
            4,
            source.base().raw() + 8,
            pdma_model_contract::abi::AbiEventStage::Source.raw(),
            pdma_model_contract::abi::AbiEventAccess::Read.raw(),
            0,
        )
    );
}

#[test]
fn strided_rows_execute_when_each_row_has_a_separate_mapping() {
    // Given: two source/destination row pairs in four separately mapped regions.
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut memory = PdmaMemoryMappings::new(provisioning.memory_access());
    let source_a = provisioning.allocate(&mut memory, 4).expect("source row 0");
    let destination_a = provisioning
        .allocate(&mut memory, 4)
        .expect("destination row 0");
    let source_b = provisioning.allocate(&mut memory, 4).expect("source row 1");
    let destination_b = provisioning
        .allocate(&mut memory, 4)
        .expect("destination row 1");
    memory
        .write_memory(source_a.base(), &[1, 2, 3, 4])
        .expect("row 0");
    memory
        .write_memory(source_b.base(), &[5, 6, 7, 8])
        .expect("row 1");
    let descriptor = AbiCommand::from_payload(
        17,
        AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
            source_dma_address: source_a.base().raw(),
            destination_dma_address: destination_a.base().raw(),
            rows: 2.into(),
            row_bytes: 4.into(),
            source_stride: 128.into(),
            destination_stride: 128.into(),
        }),
    );
    let decoded = decode(descriptor, CommandId::new(17));
    let DecodedOperation::StridedCopy(rows) = decoded.operation else {
        panic!("strided operands");
    };
    let route = CommandRoute::new(
        QueueId::new(0),
        CommandId::new(17),
        CompletionRingId::new(0),
        true,
    );
    // When: the existing datapath executes the decoded rows in order.
    let outcome = execute_strided(&memory, route, rows);
    // Then: both mapped destinations receive their corresponding source row.
    assert!(
        matches!(outcome, ExecutionOutcome::Success(_)),
        "{outcome:?}"
    );
    assert_eq!(
        memory.read_memory(destination_a.base(), 4).expect("row 0"),
        [1, 2, 3, 4]
    );
    assert_eq!(
        memory.read_memory(destination_b.base(), 4).expect("row 1"),
        [5, 6, 7, 8]
    );
}

#[test]
fn zero_rows_succeed_when_addresses_are_unmapped() {
    // Given: zero rows and addresses that cannot identify any mapped byte.
    let memory = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let payload = AbiCommandPayloadStridedCopyValues {
        source_dma_address: u64::MAX,
        destination_dma_address: u64::MAX,
        rows: 0.into(),
        row_bytes: u32::MAX.into(),
        source_stride: u32::MAX.into(),
        destination_stride: u32::MAX.into(),
    };
    // When: the decoder and existing execution unit process the descriptor.
    let outcome = decode_and_execute(&memory, payload);
    // Then: no memory operand is visited, and the operation succeeds.
    assert!(matches!(outcome, ExecutionOutcome::Success(_)));
}

#[test]
fn strided_alias_reads_observe_the_preceding_row_write() {
    // Given: the second source row aliases the first destination row.
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut memory = PdmaMemoryMappings::new(provisioning.memory_access());
    let region = provisioning.allocate(&mut memory, 12).expect("rows");
    memory
        .write_memory(region.base(), &[1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0])
        .expect("initial rows");
    let payload = AbiCommandPayloadStridedCopyValues {
        source_dma_address: region.base().raw(),
        destination_dma_address: region.base().raw() + 4,
        rows: 2.into(),
        row_bytes: 4.into(),
        source_stride: 4.into(),
        destination_stride: 4.into(),
    };
    // When: the rows execute in descriptor order.
    let outcome = decode_and_execute(&memory, payload);
    // Then: row two reads the bytes written by row one.
    assert!(matches!(outcome, ExecutionOutcome::Success(_)));
    assert_eq!(
        memory.read_memory(region.base(), 12).expect("rows"),
        [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
    );
}

fn decode_and_execute(
    memory: &PdmaMemoryMappings,
    payload: AbiCommandPayloadStridedCopyValues,
) -> ExecutionOutcome {
    let decoded = decode(
        AbiCommand::from_payload(17, AbiCommandPayloadValues::StridedCopy(payload)),
        CommandId::new(17),
    );
    let DecodedOperation::StridedCopy(rows) = decoded.operation else {
        panic!("strided operands");
    };
    execute_strided(
        memory,
        CommandRoute::new(
            QueueId::new(0),
            CommandId::new(17),
            CompletionRingId::new(0),
            true,
        ),
        rows,
    )
}
