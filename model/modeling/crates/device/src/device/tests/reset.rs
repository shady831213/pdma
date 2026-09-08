use super::super::{DeviceCore, ModelStepStatus};
use crate::test_support::{configure, copy, doorbell, submit};
use crate::{AsyncStallReason, MappedDmaRegion, PdmaMemoryProvisioning};
use pdma_model_contract::abi::registers::{GlobalRegisters, locate};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};

fn reset(core: &mut DeviceCore) {
    let register =
        locate(u32::try_from(GlobalRegisters::RESET.offset()).expect("offset")).expect("reset");
    core.write_mmio_register(register, 1).expect("reset action");
}

fn prepared(resources: ResourceConfig, count: u32) -> (DeviceCore, Vec<MappedDmaRegion>) {
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut core = DeviceCore::new(provisioning.memory_access(), resources);
    core.enable_tracing();
    let source = provisioning.allocate(&mut core.memory, 1).expect("source");
    core.memory
        .write_object(source.object(), 0, &[0x5a])
        .expect("source bytes");
    let queue = configure(&mut core, &mut provisioning, 0, [2, 1, 1]);
    let mut destinations = Vec::new();
    for command in 0..count {
        let destination = provisioning
            .allocate(&mut core.memory, 1)
            .expect("destination");
        submit(
            &mut core,
            queue,
            copy(
                u64::from(command),
                source.base().raw(),
                destination.base().raw(),
                1,
            ),
        );
        destinations.push(destination);
    }
    doorbell(&mut core, queue);
    (core, destinations)
}

#[test]
fn reset_preserves_persistent_state_and_clears_resettable_hardware() {
    // Given: one completed copy, trace history, DMA bytes, and an epoch-zero queue.
    let (mut core, destinations) = prepared(ResourceConfig::default(), 1);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    let generation = core.memory.generation();
    let trace = core.trace_events();
    // When: the sole reset runner advances the device epoch.
    reset(&mut core);
    // Then: persistent DMA/trace survive while work, IRQ, and queue identity restart.
    assert_eq!(core.epoch(), DeviceEpoch::new(1));
    assert_eq!(core.memory.generation(), generation);
    assert_eq!(
        core.memory
            .read_object(destinations.first().expect("destination").object(), 0, 1),
        Ok(vec![0x5a])
    );
    assert_eq!(core.trace_events(), trace);
    assert_eq!(core.queues.queue_count(), 0);
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
    assert_eq!(core.reset_runner_calls(), 1);
}

#[test]
fn reset_discards_resource_stalled_work_before_execution() {
    // Given: a queued copy that cannot acquire a read slot.
    let (mut core, destinations) = prepared(ResourceConfig::default().with_read_slots(0), 1);
    assert_eq!(
        core.step(),
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure
        ))
    );
    // When: reset clears the stalled queue.
    reset(&mut core);
    // Then: discarded work neither executes nor publishes after reset.
    assert_eq!(
        core.memory
            .read_object(destinations.first().expect("destination").object(), 0, 1),
        Ok(vec![0])
    );
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
    assert_eq!(core.queues.queue_count(), 0);
}

#[test]
fn reset_discards_pending_retirement_without_late_publication() {
    // Given: two executed copies with the second retained behind publication pressure.
    let (mut core, destinations) = prepared(ResourceConfig::default(), 2);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    assert_eq!(
        core.step(),
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure
        ))
    );
    // When: reset drops the cached retirement record.
    reset(&mut core);
    // Then: executed DMA effects persist but no late completion or IRQ can appear.
    for destination in destinations {
        assert_eq!(
            core.memory.read_object(destination.object(), 0, 1),
            Ok(vec![0x5a])
        );
    }
    assert_eq!(core.pending_dma_result(), None);
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
}
