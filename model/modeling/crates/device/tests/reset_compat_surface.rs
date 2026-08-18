//! Exercises the public compatibility reset and layered error surface.

#[path = "reset_compat_surface/clearing.rs"]
pub mod clearing;
/// Isolated real public overflow-reset proof for the compatibility surface.
#[path = "reset_compat_surface/overflow.rs"]
pub mod overflow;
#[path = "reset_compat_surface/support.rs"]
pub mod support;

use std::error::Error;
use std::sync::Arc;

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::semantic::{
    DeviceEpoch, IrqRoute, IrqVector, ResourceConfig, WholeDeviceReset,
};
use pdma_model_device::compat::SyncDevice;
use pdma_model_device::{
    AsyncDevice, MmioBatchError, ModelError, NoopRegisterObserver, PdmaControlError,
    PdmaMemoryError, QueueBootstrapBatch, SharedMemory,
};
use support::{
    RecordingObserver, TestResult, activation, core, queue_config, raw_reset, read_u64,
    require_error, submit_copy,
};

#[tokio::test(flavor = "current_thread")]
async fn reset_compat_surface() -> TestResult {
    // Given: exact and semantic facades with independently injected core capabilities.
    let exact_memory = Arc::new(SharedMemory::new());
    let exact_observer = Arc::new(RecordingObserver::default());
    let (exact_device, _worker) = AsyncDevice::from_core(core(
        Arc::clone(&exact_memory),
        ResourceConfig::default(),
        exact_observer.clone(),
    ));
    let exact = exact_device.handle();
    let exact_endpoint = exact.register_endpoint();
    let exact_region = exact.allocate_memory_region(4).await?;
    exact
        .write_memory(exact_region.base(), &[1, 2, 3, 4])
        .await?;
    let exact_mappings = exact_memory.regions();
    let exact_generation = exact.memory_generation().await;
    let mut before = [0_u8; 4];
    exact_endpoint.read(0, &mut before).await?;
    support::write_u64(&exact, 0x010, 9).await?;
    support::write_u64(&exact, 0x108, 7).await?;
    support::write_u64(&exact, 0x208, 5).await?;

    // When: exact register reset and semantic compatibility reset both commit.
    raw_reset(&exact, 0).await?;
    let mut after = [0_u8; 4];
    exact_endpoint.read(0, &mut after).await?;

    let compat_memory = Arc::new(SharedMemory::new());
    let adverse_observer = Arc::new(RecordingObserver::default());
    let mut compat = SyncDevice::from_core(core(
        Arc::clone(&compat_memory),
        ResourceConfig::default(),
        adverse_observer.clone(),
    ));
    compat.enable_tracing();
    let old_queue = compat.create_queue(queue_config())?;
    let old_source = old_queue.completion_irq_source();
    compat.configure_irq_route(old_source, IrqRoute::Message(IrqVector::new(7)))?;
    compat.enable_irq_source(old_source)?;
    compat.unmask_irq_source(old_source)?;
    let input = compat.allocate_buffer(1)?;
    let output = compat.allocate_buffer(1)?;
    compat.write_buffer(input, 0, &[0x6d])?;
    submit_copy(&mut compat, old_queue, input, output, 0)?;
    compat.run_until_idle(1)?;
    let notification = compat
        .wait_irq_notification()
        .ok_or("message notification must be present")?;
    let compat_bytes = compat.read_buffer(output, 0, 1)?;
    let compat_mappings = compat_memory.regions();
    let compat_generation = compat.memory_generation();
    let compat_resources = compat.resource_config();
    let compat_trace = compat.trace_events();
    let compat_lifecycle = compat.lifecycle_steps();
    let compat_epoch = compat.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;

    // Then: both calls used one core seam and preserve/clear the pinned state partition.
    let exact_reset_calls = exact.reset_runner_calls().await;
    let compat_reset_calls = compat.reset_runner_calls();
    let shared_reset_runner = exact_reset_calls == 1 && compat_reset_calls == 1;
    let observations = exact_observer.snapshot();
    let (first_sequence, second_sequence) = observation_sequences(&observations)?;
    let bytes_persist = exact.read_memory(exact_region.base(), 4).await? == [1, 2, 3, 4]
        && compat.read_buffer(output, 0, 1)? == compat_bytes;
    let mappings_persist =
        exact_memory.regions() == exact_mappings && compat_memory.regions() == compat_mappings;
    let generation_persist = exact.memory_generation().await == exact_generation
        && compat.memory_generation() == compat_generation;
    let resources_persist = compat.resource_config() == compat_resources;
    let trace_persist = compat.trace_events() == compat_trace;
    let lifecycle_persist = compat.lifecycle_steps() == compat_lifecycle;
    let fresh_queue = compat.create_queue(queue_config())?;
    let queues_cleared = matches!(
        compat.ring_doorbell(old_queue),
        Err(ModelError::StaleDeviceEpoch { .. })
    ) && fresh_queue.queue_id().raw() == 0;
    let results_cleared = compat.poll_completion(fresh_queue)?.is_none();
    let irq_cleared = compat.irq_snapshot().is_none();
    let registers_reset = read_u64(&exact, 0x010).await? == 0
        && read_u64(&exact, 0x108).await? == 0
        && read_u64(&exact, 0x208).await? == 0;
    let epochs_advanced =
        read_u64(&exact, 0x050).await? == 1 && compat_epoch == DeviceEpoch::new(1);

    let (work_cleared, retirement_cleared) = clearing::observe()?;

    assert_eq!([first_sequence, second_sequence], [0, 1]);
    assert!(shared_reset_runner && bytes_persist && mappings_persist && generation_persist);
    assert!(resources_persist && trace_persist && lifecycle_persist && queues_cleared);
    assert!(results_cleared && work_cleared && retirement_cleared && irq_cleared);
    assert!(registers_reset && epochs_advanced);
    println!(
        "exact_reset_calls={exact_reset_calls};compat_reset_calls={compat_reset_calls};shared_reset_runner={shared_reset_runner};history={};sequences={},{};bytes_persist={bytes_persist};mappings_persist={mappings_persist};generation_persist={generation_persist};resources_persist={resources_persist};trace_persist={trace_persist};lifecycle_persist={lifecycle_persist};queues_cleared={queues_cleared};results_cleared={results_cleared};work_cleared={work_cleared};retirement_cleared={retirement_cleared};irq_cleared={irq_cleared};registers_reset={registers_reset};epochs_advanced={epochs_advanced}",
        observations.len(),
        first_sequence,
        second_sequence
    );

    adverse_surface(
        &mut compat,
        old_queue,
        old_source,
        notification.source_capability(),
        &adverse_observer,
    )?;
    error_sources().await?;
    Ok(())
}

fn observation_sequences(
    observations: &[pdma_model_device::RegisterObservation],
) -> TestResult<(u64, u64)> {
    let [first, second] = observations else {
        return Err("exactly two observations required".into());
    };
    Ok((first.sequence, second.sequence))
}

fn adverse_surface(
    device: &mut SyncDevice,
    queue: pdma_model_contract::semantic::QueueConfigHandle,
    irq: pdma_model_contract::semantic::EpochIrqSource,
    notification: pdma_model_contract::semantic::EpochIrqSource,
    observer: &Arc<RecordingObserver>,
) -> TestResult {
    let bytes = device.read_buffer(pdma_model_contract::semantic::BufferId::new(1), 0, 1)?;
    let mappings = device.mapped_region_count();
    let generation = device.memory_generation();
    let resources = device.resource_config();
    let trace = device.trace_events();
    let lifecycle = device.lifecycle_steps();
    let stale_queue = matches!(
        device.ring_doorbell(queue),
        Err(ModelError::StaleDeviceEpoch { .. })
    );
    let stale_irq = matches!(
        device.enable_irq_source(irq),
        Err(ModelError::StaleDeviceEpoch { .. })
    );
    let stale_notification = matches!(
        device.ack_irq_source(notification),
        Err(ModelError::StaleDeviceEpoch { .. })
    );
    let (epoch_overflow, mutations) = overflow::run()?;
    let bytes_unchanged =
        device.read_buffer(pdma_model_contract::semantic::BufferId::new(1), 0, 1)? == bytes;
    let mappings_unchanged = device.mapped_region_count() == mappings;
    let generation_unchanged = device.memory_generation() == generation;
    let resources_unchanged = device.resource_config() == resources;
    let trace_unchanged = device.trace_events() == trace;
    let lifecycle_unchanged = device.lifecycle_steps() == lifecycle;
    let observations = observer.snapshot().len();
    assert!(stale_queue && stale_irq && stale_notification && epoch_overflow);
    assert_eq!(mutations, 0);
    assert_eq!(observations, 0);
    println!(
        "stale_queue={stale_queue};stale_irq={stale_irq};stale_notification={stale_notification};epoch_overflow={epoch_overflow};mutations={mutations};observations={observations};bytes_unchanged={bytes_unchanged};mappings_unchanged={mappings_unchanged};generation_unchanged={generation_unchanged};resources_unchanged={resources_unchanged};trace_unchanged={trace_unchanged};lifecycle_unchanged={lifecycle_unchanged}"
    );
    Ok(())
}

async fn error_sources() -> TestResult {
    let (device, _worker) = AsyncDevice::from_core(core(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let handle = device.handle();
    let uncommitted = handle.stage_memory_regions(&[128, 32, 64]).await?;
    let uncommitted_activation = activation(uncommitted.regions(), DeviceEpoch::initial())?;
    let generation = handle.memory_generation().await;
    let control = require_error(
        handle.activate_mapped_queue(uncommitted_activation).await,
        "uncommitted mapping did not reject activation",
    )?;
    let PdmaControlError::Dma(memory) = control else {
        return Err("stale activation did not preserve its DMA cause".into());
    };
    assert_eq!(memory, PdmaMemoryError::InvalidRegion);
    let control_source = control
        .source()
        .and_then(|source| source.downcast_ref::<PdmaMemoryError>());
    assert_eq!(control_source, Some(&memory));
    assert_eq!(handle.memory_generation().await, generation);

    let canonical = handle.stage_memory_regions(&[128, 32, 64]).await?;
    let canonical_activation = activation(canonical.regions(), DeviceEpoch::initial())?;
    let short = handle.stage_memory_regions(&[128, 32]).await?;
    let batch = require_error(
        handle
            .bootstrap_queue_batch(QueueBootstrapBatch::new(short, canonical_activation))
            .await,
        "invalid batch shape was accepted",
    )?;
    assert_eq!(batch, MmioBatchError::InvalidShape);
    assert!(batch.source().is_none());

    let mut run = SyncDevice::from_core(core(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
        Arc::new(NoopRegisterObserver),
    ));
    let queue = run.create_queue(queue_config())?;
    let source = run.allocate_buffer(1)?;
    let destination = run.allocate_buffer(1)?;
    submit_copy(&mut run, queue, source, destination, 0)?;
    let run_error = require_error(
        run.run_until_idle_typed(0),
        "zero budget unexpectedly completed pending work",
    )?;
    assert_eq!(
        run_error.source().and_then(|source| source.downcast_ref()),
        Some(&ModelError::StepBudgetExhausted { max_steps: 0 })
    );

    let reset_epoch = run.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    let calls = run.reset_runner_calls();
    let reset = require_error(
        run.reset(WholeDeviceReset::new(DeviceEpoch::initial())),
        "stale reset unexpectedly committed",
    )?;
    assert_eq!(reset_epoch, DeviceEpoch::new(1));
    let reset_source = reset
        .source()
        .and_then(|source| source.downcast_ref::<PdmaControlError>());
    assert_eq!(
        reset_source,
        Some(&PdmaControlError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    assert_eq!(run.reset_runner_calls(), calls);
    Ok(())
}
