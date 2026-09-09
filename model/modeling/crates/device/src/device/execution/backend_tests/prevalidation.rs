use crate::test_support::{allocate, configure, doorbell, poll_completion, poll_event, submit};
use crate::{DeviceCore, ModelError};
use hardware_interface_runtime::{MemoryAddress, SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadValues, AbiEventAccess,
    AbiEventKind, AbiEventStage,
};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

#[test]
fn broadcast_preserves_first_destination_when_second_extent_is_invalid() {
    // Given: a valid first destination and a short second destination in external memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let mut core = DeviceCore::new(allocator.memory_access(), ResourceConfig::default());
    let queue = configure(&mut core, &allocator, 0, [2, 2, 2]);
    let [source, first, second, list] = [4, 4, 2, 16].map(|length| allocate(&allocator, length));
    crate::memory::write(&core.memory, source.base(), &[1, 2, 3, 4]).expect("source");
    crate::memory::write(&core.memory, first.base(), &[0xa5; 4]).expect("first sentinel");
    crate::memory::write(&core.memory, second.base(), &[0x5a; 2]).expect("second sentinel");
    let entries = [
        first.base().raw().to_le_bytes(),
        second.base().raw().to_le_bytes(),
    ]
    .concat();
    crate::memory::write(&core.memory, list.base(), &entries).expect("destination list");
    submit(
        &mut core,
        queue,
        AbiCommand::from_payload(
            71,
            AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
                source_dma_address: source.base().raw(),
                destination_list_dma_address: list.base().raw(),
                length: 4.into(),
                destination_count: 2.into(),
            }),
        ),
    );
    doorbell(&mut core, queue);

    // When: the second destination fails extent validation.
    let result = core.step();

    // Then: neither destination changes and retirement reports the second access precisely.
    let event = poll_event(&mut core, queue).expect("fault event");
    assert_eq!(result, Err(ModelError::MemoryFault(event.fault())));
    assert_eq!(event.command_id, 71);
    assert_eq!(
        event.fault().tuple(),
        (
            AbiEventKind::OutOfBounds,
            second.base().raw() + 2,
            2,
            second.base().raw() + 4,
            AbiEventStage::Destination.raw(),
            AbiEventAccess::Write.raw(),
            1,
        )
    );
    assert_eq!(
        crate::memory::read(&core.memory, first.base(), 4),
        Ok(vec![0xa5; 4])
    );
    assert_eq!(
        crate::memory::read(&core.memory, second.base(), 2),
        Ok(vec![0x5a; 2])
    );
    assert_eq!(poll_completion(&mut core, queue), None);
}

#[test]
fn broadcast_source_alias_reads_observe_the_preceding_destination_write() {
    // Given: the first broadcast destination overlaps its source by one byte.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let mut core = DeviceCore::new(allocator.memory_access(), ResourceConfig::default());
    let queue = configure(&mut core, &allocator, 0, [2, 2, 2]);
    let [source, destination, list] = [8, 4, 16].map(|length| allocate(&allocator, length));
    crate::memory::write(&core.memory, source.base(), &[1, 2, 3, 4, 5, 6, 7, 8]).expect("source");
    let entries = [
        (source.base().raw() + 1).to_le_bytes(),
        destination.base().raw().to_le_bytes(),
    ]
    .concat();
    crate::memory::write(&core.memory, list.base(), &entries).expect("destination list");
    submit(
        &mut core,
        queue,
        AbiCommand::from_payload(
            72,
            AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
                source_dma_address: source.base().raw(),
                destination_list_dma_address: list.base().raw(),
                length: 4.into(),
                destination_count: 2.into(),
            }),
        ),
    );
    doorbell(&mut core, queue);

    // When: broadcast visits each destination in descriptor order.
    core.step().expect("broadcast executes");

    // Then: the second destination receives the source modified by the first write.
    assert_eq!(
        crate::memory::read(&core.memory, source.base(), 8),
        Ok(vec![1, 1, 2, 3, 4, 6, 7, 8])
    );
    assert_eq!(
        crate::memory::read(&core.memory, destination.base(), 4),
        Ok(vec![1, 1, 2, 3])
    );
    assert_eq!(
        poll_completion(&mut core, queue),
        Some(pdma_model_contract::abi::AbiCompletion::success(0, 72))
    );
    assert_eq!(poll_event(&mut core, queue), None);
}
