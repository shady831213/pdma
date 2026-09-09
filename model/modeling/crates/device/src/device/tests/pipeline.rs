use super::super::{DeviceCore, ModelStepStatus};
use crate::AsyncStallReason;
use crate::test_support::{TestQueue, configure, copy, doorbell, poll_completion, submit};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues, AbiCompletionStatus,
    U32WordValues,
};
use pdma_model_contract::semantic::{CommandId, ResourceConfig, TraceEventKind};

struct Fixture {
    core: DeviceCore,
    copy_queue: TestQueue,
    second_destination: pdma_model_contract::abi::DmaAddress,
}

fn fixture() -> Fixture {
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        hardware_interface_runtime::MemoryAddress::new(0x1_0000),
    );
    let mut core = DeviceCore::new(
        allocator.memory_access(),
        ResourceConfig::default().with_read_slots(1),
    );
    core.enable_tracing();
    let copy_queue = configure(&mut core, &allocator, 0, [4, 1, 4]);
    let vector_queue = configure(&mut core, &allocator, 1, [4, 4, 4]);
    let source = crate::test_support::allocate(&allocator, 4);
    let first_destination = crate::test_support::allocate(&allocator, 4);
    let second_destination = crate::test_support::allocate(&allocator, 4);
    let vector_left = crate::test_support::allocate(&allocator, 4);
    let vector_right = crate::test_support::allocate(&allocator, 4);
    let vector_destination = crate::test_support::allocate(&allocator, 4);
    crate::memory::write(&core.memory, source.base(), &[2, 4, 6, 8]).expect("source bytes");
    for (id, destination) in [first_destination, second_destination]
        .into_iter()
        .enumerate()
    {
        submit(
            &mut core,
            copy_queue,
            copy(
                u64::try_from(id).expect("id"),
                source.base().raw(),
                destination.base().raw(),
                4,
            ),
        );
    }
    submit(
        &mut core,
        vector_queue,
        AbiCommand::from_payload(
            2,
            AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                source_a_dma_address: vector_left.base().raw(),
                source_b_dma_address: vector_right.base().raw(),
                destination_dma_address: vector_destination.base().raw(),
                length: U32WordValues { value: 4 },
            }),
        ),
    );
    doorbell(&mut core, copy_queue);
    doorbell(&mut core, vector_queue);
    Fixture {
        core,
        copy_queue,
        second_destination: second_destination.base(),
    }
}

#[test]
fn admission_skips_starved_work_without_replaying_accepted_retirement() {
    // Given: two copy commands and one vector command sharing constrained resources.
    let Fixture {
        mut core,
        copy_queue,
        second_destination,
    } = fixture();

    // When: the runnable copy reaches publication pressure before the starved vector is admitted.
    let first_copy = core.step();
    let publication_stall = core.step();
    let pending_retry = core.step();
    let first_completion = poll_completion(&mut core, copy_queue);
    let retried = core.step();
    let second_completion = poll_completion(&mut core, copy_queue);
    let resource_stall = core.step();
    let repeated_resource_stall = core.step();

    // Then: accepted retirement is retained while inadmissible work is never fetched or executed.
    assert_eq!(first_copy, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        publication_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure
        ))
    );
    assert_eq!(publication_stall, pending_retry);
    assert_eq!(retried, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        resource_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure
        ))
    );
    assert_eq!(repeated_resource_stall, resource_stall);
    assert_eq!(
        first_completion
            .map(|record| CommandId::new(usize::try_from(record.command_id).expect("id"))),
        Some(CommandId::new(0))
    );
    assert_eq!(
        second_completion.map(|record| (
            CommandId::new(usize::try_from(record.command_id).expect("id")),
            record.status
        )),
        Some((CommandId::new(1), AbiCompletionStatus::Success))
    );
    assert_eq!(
        crate::memory::read(&core.memory, second_destination, 4),
        Ok(vec![2, 4, 6, 8])
    );
    let events = core.trace_events();
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::ExecutionSucceeded
                    && event.command_id() == Some(CommandId::new(1))
            })
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::Scheduled
                    && event.command_id() == Some(CommandId::new(2))
            })
            .count(),
        0
    );
}

#[test]
fn retirement_retry_does_not_reexecute_or_consume_resource_stalled_work() {
    // Given: constrained copy retirement and a vector command that cannot acquire read slots.
    let Fixture {
        mut core,
        copy_queue,
        ..
    } = fixture();

    // When: completion pressure clears and the worker retries before encountering resource stalls.
    let first_copy = core.step();
    let completion_stall = core.step();
    let retry_before_poll = core.step();
    let first_completion = poll_completion(&mut core, copy_queue);
    let retry_after_poll = core.step();
    let second_completion = poll_completion(&mut core, copy_queue);
    let resource_stall = core.step();
    let repeated_resource_stall = core.step();

    // Then: retry neither repeats the accepted first copy nor schedules resource-stalled vector work.
    assert_eq!(first_copy, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        completion_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure
        ))
    );
    assert_eq!(retry_before_poll, completion_stall);
    assert_eq!(retry_after_poll, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        first_completion
            .map(|record| CommandId::new(usize::try_from(record.command_id).expect("id"))),
        Some(CommandId::new(0))
    );
    assert_eq!(
        second_completion
            .map(|record| CommandId::new(usize::try_from(record.command_id).expect("id"))),
        Some(CommandId::new(1))
    );
    assert_eq!(
        resource_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure
        ))
    );
    assert_eq!(repeated_resource_stall, resource_stall);
    let events = core.trace_events();
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::ExecutionSucceeded
                    && event.command_id() == Some(CommandId::new(0))
            })
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::Scheduled
                    && event.command_id() == Some(CommandId::new(2))
            })
            .count(),
        0
    );
}
