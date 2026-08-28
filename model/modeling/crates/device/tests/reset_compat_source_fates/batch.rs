use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use hardware_interface_runtime::{RegisterObservation, RegisterObserver};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_contract::semantic::RingCursor;
use pdma_model_device::test_support::{DeviceStateSnapshot, async_state_snapshot};
use pdma_model_device::{
    AsyncDeviceHandle, DmaWrite, PdmaBatchError, QueueBootstrapBatch, SubmissionBatch,
};

use super::{TestResult, require_error};

#[tokio::test(flavor = "current_thread")]
async fn batch_fate_rows_have_literal_sources() -> TestResult {
    // Given: public malformed, DMA, terminal-command, and observer-fault batch requests.
    let handle = new_handle(Arc::new(hardware_interface_runtime::NoopRegisterObserver));
    let canonical = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = super::activation(canonical.regions(), DeviceEpoch::initial())?;
    let short = handle.stage_memory_regions(&[128, 64]).await?;
    let before = async_state_snapshot(&handle).await?;
    let shape = require_error(
        handle
            .bootstrap_queue_batch(QueueBootstrapBatch::new(short, activation))
            .await,
    )?;
    assert_eq!(shape, PdmaBatchError::InvalidShape);
    assert!(shape.source().is_none());
    assert_state_unchanged(&handle, &before).await?;

    let queue = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(canonical, activation))
        .await?;
    assert_eq!(queue.as_bytes().len(), 72);
    let dma_before = async_state_snapshot(&handle).await?;
    let dma = require_error(
        handle
            .submit_queue_batch(SubmissionBatch::new(
                activation.queue(),
                RingCursor::new(0),
                DmaWrite::new(DmaAddress::new(0xdead_0000), vec![0; 64]),
                None,
            ))
            .await,
    )?;
    assert!(matches!(
        dma,
        PdmaBatchError::Dma(details)
            if details.result == pdma_model_contract::abi::mmio::MmioResult::DmaUnmapped
    ));
    assert!(dma.source().is_none());
    assert_state_unchanged(&handle, &dma_before).await?;

    let command_before = async_state_snapshot(&handle).await?;
    let command = require_error(
        handle
            .submit_queue_batch(SubmissionBatch::new(
                activation.queue(),
                RingCursor::new(1),
                DmaWrite::new(DmaAddress::new(0x1_0000), vec![0; 64]),
                None,
            ))
            .await,
    )?;
    assert!(matches!(
        command,
        PdmaBatchError::Command(details)
            if details.result == pdma_model_contract::abi::mmio::MmioResult::InvalidTransaction
    ));
    assert!(command.source().is_none());
    assert_state_unchanged(&handle, &command_before).await?;

    assert_observation_source().await
}

async fn assert_observation_source() -> TestResult {
    let observer = Arc::new(PanicObserver::new());
    let observer_for_handle: Arc<dyn RegisterObserver> = observer.clone();
    let observer_handle = new_handle(observer_for_handle);
    let stage = observer_handle
        .stage_memory_regions(&[128, 64, 128])
        .await?;
    let observer_activation = super::activation(stage.regions(), DeviceEpoch::initial())?;
    let observation_before = async_state_snapshot(&observer_handle).await?;
    let observation = require_error(
        observer_handle
            .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, observer_activation))
            .await,
    )?;
    assert_eq!(
        observation
            .source()
            .and_then(|source| source.downcast_ref()),
        Some(&hardware_interface_runtime::ObservationGateError::Faulted {
            reason: hardware_interface_runtime::GateFault::ObserverPanicked,
        })
    );
    let observation_after = async_state_snapshot(&observer_handle).await?;
    assert_ne!(observation_after.mappings, observation_before.mappings);
    assert_ne!(observation_after.bytes, observation_before.bytes);
    assert_eq!(
        observation_after.memory_generation,
        observation_before.memory_generation + 1
    );
    assert_eq!(observation_after.resources, observation_before.resources);
    assert_eq!(observation_after.trace, observation_before.trace);
    assert_eq!(
        observation_after.runner.steps,
        observation_before.runner.steps
    );
    assert_eq!(
        observation_after.runner.errors,
        observation_before.runner.errors
    );
    assert_eq!(
        observation_after.runner.stalls,
        observation_before.runner.stalls
    );
    assert_eq!(
        observation_after.runner.last_status,
        observation_before.runner.last_status
    );
    assert_eq!(
        observation_after.runner.shutdown_requested,
        observation_before.runner.shutdown_requested
    );
    assert_eq!(
        observation_after.runner_generation,
        observation_before.runner_generation + 1
    );
    assert_eq!(observation_after.partitions.queues, 1);
    assert_eq!(observation_after.partitions.results, 0);
    assert_eq!(observation_after.partitions.work, 0);
    assert_eq!(observation_after.partitions.retirement, 0);
    assert_eq!(observation_after.partitions.irq_sources, 0);
    assert_ne!(
        observation_after.partitions.registers,
        observation_before.partitions.registers
    );
    assert_eq!(
        observation_after.partitions.backend_epoch,
        observation_before.partitions.backend_epoch
    );
    assert_eq!(
        observation_after.partitions.control_epoch,
        observation_before.partitions.control_epoch
    );
    assert_eq!(observation_after.irq, observation_before.irq);
    assert_eq!(
        observation_after.reset_runner_calls,
        observation_before.reset_runner_calls
    );
    assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

fn new_handle(observer: Arc<dyn RegisterObserver>) -> pdma_model_device::AsyncDeviceHandle {
    let (device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            observer,
        ));
    device.handle()
}

async fn assert_state_unchanged(
    handle: &AsyncDeviceHandle,
    before: &DeviceStateSnapshot,
) -> TestResult {
    let after = async_state_snapshot(handle).await?;
    assert_eq!(after.mappings, before.mappings);
    assert_eq!(after.bytes, before.bytes);
    assert_eq!(after.memory_generation, before.memory_generation);
    assert_eq!(after.resources, before.resources);
    assert_eq!(after.trace, before.trace);
    assert_eq!(after.runner, before.runner);
    assert_eq!(after.runner_generation, before.runner_generation);
    assert_eq!(after.partitions.queues, before.partitions.queues);
    assert_eq!(after.partitions.results, before.partitions.results);
    assert_eq!(after.partitions.work, before.partitions.work);
    assert_eq!(after.partitions.retirement, before.partitions.retirement);
    assert_eq!(after.partitions.irq_sources, before.partitions.irq_sources);
    assert_eq!(after.partitions.registers, before.partitions.registers);
    assert_eq!(
        after.partitions.backend_epoch,
        before.partitions.backend_epoch
    );
    assert_eq!(
        after.partitions.control_epoch,
        before.partitions.control_epoch
    );
    assert_eq!(after.irq, before.irq);
    assert_eq!(after.reset_runner_calls, before.reset_runner_calls);
    Ok(())
}

struct PanicObserver {
    calls: AtomicUsize,
}

impl PanicObserver {
    const fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}

impl RegisterObserver for PanicObserver {
    fn observe(&self, _: RegisterObservation) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::panic::resume_unwind(Box::new("required observation-failure path"));
    }
}
