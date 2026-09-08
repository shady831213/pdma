use crate::DeviceCore;
use crate::queue::QueueBlock;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{
    GlobalResponseBank, MmioResult, MmioResultDetails, QueueResponseBank, ResponseUpdate,
    register_at,
};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

fn completion(result: MmioResult, details: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails { result, details }
}

#[test]
fn command_completion_replaces_or_preserves_atomically() {
    let mut queues = QueueBlock::default();
    queues.complete(
        completion(MmioResult::Ok, [1, 2, 3]),
        ResponseUpdate::Replace(QueueResponseBank {
            epoch: 0xfeed_u64.into(),
            ..QueueResponseBank::default()
        }),
    );
    queues.complete(
        completion(MmioResult::InvalidTransaction, [4, 5, 6]),
        ResponseUpdate::Preserve,
    );
    assert_eq!(
        queues.registers.result.raw(),
        MmioResult::InvalidTransaction.code()
    );
    assert_eq!(queues.registers.response.epoch.raw(), 0xfeed);
}

#[test]
fn empty_completion_zero_fills_the_response() {
    let mut queues = QueueBlock::default();
    queues.complete(
        completion(MmioResult::Ok, [0; 3]),
        ResponseUpdate::Replace(QueueResponseBank {
            epoch: 0xfeed_u64.into(),
            ..QueueResponseBank::default()
        }),
    );
    queues.complete(completion(MmioResult::Empty, [0; 3]), ResponseUpdate::Empty);
    assert_eq!(queues.registers.result.raw(), MmioResult::Empty.code());
    assert_eq!(queues.registers.response.epoch.raw(), 0);
}

#[test]
fn global_owner_preserves_compact_response_encoding() {
    // Given: one global owner and distinct values for each response field.
    let mut core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let response = GlobalResponseBank {
        abi_version: 1.into(),
        run_state: 2.into(),
        features: 3.into(),
        epoch: 4.into(),
        max_ring_depth: 5.into(),
        max_indirect_destinations: 6.into(),
        max_transfer_bytes: 7.into(),
        max_strided_rows: 8.into(),
        max_stride: 9.into(),
    };
    let offsets = [
        0x040, 0x044, 0x048, 0x050, 0x058, 0x05c, 0x060, 0x064, 0x068,
    ];
    // When: the global owner receives the complete typed response.
    core.global_registers.response = response;
    // Then: raw leaf bytes exactly match the existing compact response codec.
    let raw: Vec<u8> = offsets
        .into_iter()
        .flat_map(|offset| {
            core.read_register(register_at(offset).expect("global response register"))
                .expect("global response bytes")
        })
        .collect();
    assert_eq!(
        raw,
        response.to_bytes().expect("global response").as_slice()
    );
}
