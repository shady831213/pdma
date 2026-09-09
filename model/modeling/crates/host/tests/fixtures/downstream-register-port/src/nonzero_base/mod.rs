use hardware_interface_runtime::{
    InterruptSource, RegisterBusAddress, RegisterBusBase, RegisterObservation, RegisterObserver,
    SharedMemory, SoftwareCpu,
};
use pdma_model_contract::semantic::{
    CommandId, EpochQueue, IrqNotification, IrqRoute, IrqVector, ResourceConfig,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    Command, CommandOperation, CopyRequest, EventRouting, QueueConfig, SoftwareOwnerId,
};
use pdma_model_host::{
    DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::TestResult;

#[derive(Debug, Default)]
struct Observer(Mutex<Vec<RegisterObservation>>);

impl RegisterObserver for Observer {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    copied: Vec<u8>,
    message: bool,
    completion: bool,
    retired: u64,
    offsets: Vec<u32>,
    addresses: Vec<RegisterBusAddress>,
}

#[tokio::test(flavor = "current_thread")]
async fn nonzero_base_routes_canonical_offsets_as_full_addresses() -> TestResult {
    // Given: independent real devices rooted at zero and above the u32 address range.
    let zero_base = RegisterBusBase::new(0);
    let nonzero_base = RegisterBusBase::new(0x1_0000_0000);

    // When: both external consumers complete queue, copy, IRQ claim, and retirement flows.
    let zero = run_case(zero_base).await?;
    let nonzero = run_case(nonzero_base).await?;

    // Then: only the full-address coordinate changes by the configured base.
    assert_eq!(zero.copied, [3, 1, 4, 1]);
    assert!(zero.message && zero.completion && zero.retired == 1);
    assert_eq!(zero.copied, nonzero.copied);
    assert_eq!(zero.message, nonzero.message);
    assert_eq!(zero.completion, nonzero.completion);
    assert_eq!(zero.retired, nonzero.retired);
    assert_eq!(zero.offsets, nonzero.offsets);
    assert!(
        nonzero
            .addresses
            .iter()
            .zip(&zero.addresses)
            .all(|(address, zero)| address.raw() == nonzero_base.raw() + zero.raw())
    );
    println!(
        "nonzero_base=true;base0_complete=true;base_0x1_0000_0000_complete=true;copied=3,1,4,1;irq=message;retired=1;address_count={};address_transcript={}",
        nonzero.addresses.len(),
        format_addresses(&nonzero.addresses)
    );
    Ok(())
}

pub(crate) async fn run_case(mmio_base: RegisterBusBase) -> TestResult<Outcome> {
    let observer = Arc::new(Observer::default());
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new_observed(
        memory.memory_access(),
        ResourceConfig::default(),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let access = AsyncDeviceAccess::new_at_mmio_base(handle.clone(), memory.clone(), mmio_base);
    assert_eq!(access.mmio_base(), mmio_base);
    let cpu = SoftwareCpu::new();
    let (notification, completion, copied, retired) = cpu
        .run_main(async {
            let mut interrupts = access.clone();
            let mut port = PdmaRegisterPort::new(access)?;
            let queue = DriverPort::create_queue(
                &mut port,
                QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
            )
            .await?;
            let irq = queue.completion_irq_source();
            DriverPort::configure_irq_route(&mut port, irq, IrqRoute::Message(IrqVector::new(73)))
                .await?;
            DriverPort::enable_irq_source(&mut port, irq).await?;
            DriverPort::unmask_irq_source(&mut port, irq).await?;
            let source = DriverPort::allocate_buffer(&mut port, 4).await?;
            let destination = DriverPort::allocate_buffer(&mut port, 4).await?;
            DriverPort::write_buffer(&mut port, source, 0, &[3, 1, 4, 1]).await?;
            DriverPort::submit_command(
                &mut port,
                queue,
                Command::new(
                    CommandId::new(0),
                    CommandOperation::Copy(CopyRequest::new(source, destination, 4)),
                ),
            )
            .await?;
            DriverPort::ring_doorbell(&mut port, queue).await?;
            let ready = interrupts.wait_ready().await?;
            let notification = interrupts
                .claim(ready)
                .await?
                .map(|notification| notification.notification());
            let completion = DriverPort::poll_completion(&mut port, queue).await?;
            let copied = DriverPort::read_buffer(&port, destination, 0, 4).await?;
            let status = port
                .queue_status(EpochQueue::new(queue.epoch(), queue.queue_id()))
                .await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((
                notification,
                completion,
                copied,
                status.completion().consumer().raw(),
            ))
        })
        .await?;
    device.request_shutdown().await;
    worker.await?;
    let offsets = lock(&observer.0)
        .iter()
        .map(|observation| observation.offset)
        .collect::<Vec<_>>();
    let addresses = offsets
        .iter()
        .map(|offset| RegisterBusAddress::new(mmio_base.raw() + u64::from(*offset)))
        .collect();
    assert_eq!(copied, [3, 1, 4, 1]);
    assert!(matches!(notification, Some(IrqNotification::Message(_))));
    assert!(completion.is_some());
    assert_eq!(retired, 1);
    Ok(Outcome {
        copied,
        message: matches!(notification, Some(IrqNotification::Message(_))),
        completion: completion.is_some(),
        retired,
        offsets,
        addresses,
    })
}

#[cfg(test)]
fn format_addresses(addresses: &[RegisterBusAddress]) -> String {
    addresses
        .iter()
        .map(|address| format!("{:#x}", address.raw()))
        .collect::<Vec<_>>()
        .join(",")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
