use hardware_interface_runtime::{MemoryAddress, RegisterAccess, SharedMemoryAllocator};
use hardware_interface_runtime::{RegisterObservation, RegisterObserver};
use pdma_model_contract::abi::registers::{
    IrqSourceRegisters, QueueRegisters, irq_offset, queue_offset,
};
use pdma_model_contract::abi::{AbiCommand, AbiCompletion, AbiEvent, DmaAddress};
use pdma_model_device::AsyncDeviceHandle;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// Surface-test error result.
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Default)]
/// Clone-safe observation recorder.
pub(super) struct RecordingObserver(Mutex<Vec<RegisterObservation>>);

impl RecordingObserver {
    /// Returns the durable observations recorded so far.
    pub(super) fn snapshot(&self) -> Vec<RegisterObservation> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl RegisterObserver for RecordingObserver {
    fn observe(&self, observation: RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[derive(Debug)]
pub(super) struct CoreContention {
    release: mpsc::Sender<()>,
    memory_holder: thread::JoinHandle<TestResult>,
    core_holder: thread::JoinHandle<TestResult>,
}

impl CoreContention {
    pub(super) fn release(self) -> TestResult {
        self.release.send(())?;
        self.memory_holder
            .join()
            .map_err(|_| "memory contention thread panicked")??;
        self.core_holder
            .join()
            .map_err(|_| "core contention thread panicked")??;
        Ok(())
    }
}

pub(super) async fn contend_core(
    handle: &AsyncDeviceHandle,
    memory: Arc<hardware_interface_runtime::SharedMemory>,
) -> TestResult<CoreContention> {
    let allocator = SharedMemoryAllocator::new(memory.clone(), MemoryAddress::new(0x10_0000));
    let address = DmaAddress::new(
        allocator
            .allocate(1, hardware_interface_runtime::MemoryAlignment::try_new(64)?)?
            .raw(),
    );
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let memory_holder = thread::spawn(move || -> TestResult {
        memory.transaction(|_| -> TestResult {
            held_tx.send(())?;
            release_rx.recv()?;
            Ok(())
        })
    });
    held_rx.recv()?;
    let contender = handle.clone();
    let core_holder = thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(contender.write_memory(address, &[0]))
            .map_err(Into::into)
    });
    wait_until_core_contended(handle).await?;
    Ok(CoreContention {
        release: release_tx,
        memory_holder,
        core_holder,
    })
}

async fn wait_until_core_contended(handle: &AsyncDeviceHandle) -> TestResult {
    for _ in 0..10_000 {
        let probe_handle = handle.clone();
        let probe = tokio::spawn(async move { probe_handle.snapshot().await });
        tokio::task::yield_now().await;
        if !probe.is_finished() {
            probe.abort();
            return Ok(());
        }
        probe.await?;
    }
    Err("public memory write never acquired the core lock".into())
}

pub(super) async fn create_message(
    handle: &AsyncDeviceHandle,
    allocator: &SharedMemoryAllocator,
) -> TestResult<DmaAddress> {
    let endpoint = handle.register_endpoint();
    let mut bases = Vec::new();
    for size in [
        AbiCommand::RECORD_SIZE,
        AbiCompletion::RECORD_SIZE,
        AbiEvent::RECORD_SIZE,
    ] {
        bases.push(DmaAddress::new(
            allocator
                .allocate(
                    u64::try_from(size)?,
                    hardware_interface_runtime::MemoryAlignment::try_new(64)?,
                )?
                .raw(),
        ));
    }
    for (local, base) in [
        QueueRegisters::SQ_BASE.offset(),
        QueueRegisters::CQ_BASE.offset(),
        QueueRegisters::EQ_BASE.offset(),
    ]
    .into_iter()
    .zip(&bases)
    {
        endpoint
            .write(queue_offset(0, local)?, &base.raw().to_le_bytes())
            .await?;
    }
    for local in [
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
        QueueRegisters::ENABLE.offset(),
    ] {
        endpoint
            .write(queue_offset(0, local)?, &1_u32.to_le_bytes())
            .await?;
    }
    endpoint
        .write(
            irq_offset(0, 0, IrqSourceRegisters::ROUTE_VECTOR.offset())?,
            &41_u64.to_le_bytes(),
        )
        .await?;
    for (local, value) in [
        (IrqSourceRegisters::ROUTE_KIND.offset(), 1_u32),
        (IrqSourceRegisters::ENABLED.offset(), 1),
        (IrqSourceRegisters::MASKED.offset(), 0),
    ] {
        endpoint
            .write(irq_offset(0, 0, local)?, &value.to_le_bytes())
            .await?;
    }
    Ok(*bases.first().ok_or("submission base")?)
}

pub(super) async fn submit_copy(
    handle: &AsyncDeviceHandle,
    allocator: &SharedMemoryAllocator,
    submission: DmaAddress,
) -> TestResult {
    use pdma_model_contract::abi::{AbiCommandPayloadCopyValues, AbiCommandPayloadValues};
    let source =
        allocator.allocate(1, hardware_interface_runtime::MemoryAlignment::try_new(64)?)?;
    let destination =
        allocator.allocate(1, hardware_interface_runtime::MemoryAlignment::try_new(64)?)?;
    let descriptor = AbiCommand::from_payload(
        0,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source.raw(),
            destination_dma_address: destination.raw(),
            length: 1.into(),
        }),
    );
    let mut bytes = vec![0; AbiCommand::RECORD_SIZE];
    descriptor.encode(&mut bytes)?;
    handle.write_memory(submission, &bytes).await?;
    for local in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::DOORBELL.offset(),
    ] {
        handle
            .register_endpoint()
            .write(queue_offset(0, local)?, &1_u64.to_le_bytes())
            .await?;
    }
    Ok(())
}

pub(super) async fn claim(handle: &AsyncDeviceHandle) -> TestResult<bool> {
    Ok(handle
        .take_irq_notification_at_epoch(pdma_model_contract::semantic::DeviceEpoch::initial())
        .await?
        .is_some())
}
