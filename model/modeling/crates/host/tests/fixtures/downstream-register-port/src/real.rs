use hardware_interface_runtime::{
    AccessOrigin, MemoryAccess, MemoryAlignment, RegisterObservation, RegisterObserver,
    SharedMemory,
};
use pdma_model_contract::semantic::{
    Command, CommandId, CopyRequest, EventRouting, IrqNotification, IrqRoute, IrqVector,
    QueueConfig, ResourceConfig, SoftwareOwnerId,
};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::adapter::abi::AsyncDeviceAccess;
use pdma_model_host::adapter::abi::{
    PdmaExactInterruptClaim, PdmaMemoryProvisioning, PdmaRegionRequest,
};
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct RealResult {
    pub nonempty_storage: bool,
    pub zero_storage: bool,
    pub notification_is_message: bool,
    pub claim_observations: usize,
    pub direct_origin: bool,
    pub observations: Vec<RegisterObservation>,
}

#[derive(Debug, Default)]
struct Observer(Mutex<Vec<RegisterObservation>>);
impl RegisterObserver for Observer {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

pub async fn run(alignment: MemoryAlignment) -> TestResult<RealResult> {
    let observer = Arc::new(Observer::default());
    let (device, worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default()),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let access = AsyncDeviceAccess::new(handle.clone());
    let nonempty = access
        .allocate_region(PdmaRegionRequest::new(8, alignment))
        .await?;
    let zero = access
        .allocate_region(PdmaRegionRequest::new(0, alignment))
        .await?;
    let storage = nonempty
        .storage()
        .ok_or("adapter nonempty storage missing")?;
    let nonempty_storage = (storage.base(), storage.length(), storage.alignment())
        == (nonempty.base(), nonempty.length(), nonempty.alignment());
    let mut empty = [];
    MemoryAccess::read(&access, zero.base(), alignment, &mut empty)?;
    assert!(
        MemoryAccess::read(
            &access,
            zero.base().checked_add(1).ok_or("zero base overflow")?,
            alignment,
            &mut empty
        )
        .is_err()
    );
    assert_eq!(handle.memory_generation().await, 2);
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
    let src = handle.allocate_buffer(1).await?;
    let dst = handle.allocate_buffer(1).await?;
    handle
        .submit_command(
            queue,
            Command::new(
                CommandId::new(0),
                pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                    src, dst, 1,
                )),
            ),
        )
        .await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;
    let notification = access.claim_interrupt_exact().await?;
    let observations = lock(&observer.0).clone();
    device.request_shutdown().await;
    worker.await?;
    Ok(RealResult {
        nonempty_storage,
        zero_storage: zero.storage().is_some(),
        notification_is_message: matches!(
            notification,
            pdma_model_host::adapter::abi::PdmaClaimResult::Notification {
                notification: IrqNotification::Message(_),
                ..
            }
        ),
        claim_observations: observations.len(),
        direct_origin: observations
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct),
        observations,
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests;
