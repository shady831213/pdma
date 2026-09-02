use hardware_interface_runtime::{RegisterObservation, RegisterObserver};
use pdma_model_contract::semantic::{
    Command, CommandId, CopyRequest, EventRouting, IrqRoute, IrqVector, QueueConfig,
    SoftwareOwnerId,
};
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
    core_holder: thread::JoinHandle<TestResult<Vec<u8>>>,
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
    let object = handle.allocate_buffer(1).await?;
    let address = handle
        .region_for_object(object)
        .await
        .ok_or("allocated object has no region")?
        .base();
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
            .block_on(contender.read_memory(address, 1))
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
    Err("public memory read never acquired the core lock".into())
}

/// Creates and enables one message-routed completion source.
pub(super) async fn create_message(
    handle: &AsyncDeviceHandle,
) -> TestResult<pdma_model_contract::semantic::QueueConfigHandle> {
    let queue = handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?;
    let source = queue.completion_irq_source();
    handle
        .configure_irq_route(source, IrqRoute::Message(IrqVector::new(41)))
        .await?;
    handle.enable_irq_source(source).await?;
    handle.unmask_irq_source(source).await?;
    Ok(queue)
}

/// Submits one copy that produces a completion message.
pub(super) async fn submit_copy(
    handle: &AsyncDeviceHandle,
    queue: pdma_model_contract::semantic::QueueConfigHandle,
) -> TestResult {
    let source = handle.allocate_buffer(1).await?;
    let destination = handle.allocate_buffer(1).await?;
    handle
        .submit_command(
            queue,
            Command::new(
                CommandId::new(0),
                pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                    source,
                    destination,
                    1,
                )),
            ),
        )
        .await?;
    Ok(())
}
