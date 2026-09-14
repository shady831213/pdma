use super::Model;
mod notifications;
use hardware_interface_pipeline::{AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use pdma_model_contract::{ChannelRegisters, CommandRegisters, DeviceRegisters, ResponseRegisters};
use pdma_model_device::{Device, Error, Fault};
use std::{
    future::Future,
    pin::{Pin, pin},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const IRQ_ENABLE: usize = (DeviceRegisters::CHANNELS_OFFSET
    + pdma_model_contract::ChannelBlock::CONTROL.offset())
    + ChannelRegisters::IRQ_ENABLE.offset();

#[derive(Default)]
struct WakeFlag(AtomicBool);

impl Wake for WakeFlag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn poll_once<F: Future>(future: Pin<&mut F>, flag: &Arc<WakeFlag>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(flag));
    future.poll(&mut Context::from_waker(&waker))
}

#[tokio::test]
async fn write_wakes_a_parked_worker_and_preserves_register_value() -> TestResult {
    // Given: a worker parked before a register transaction.
    let model = Model::new(Device::new((), 1)?);
    let flag = Arc::new(WakeFlag::default());
    let mut wait = pin!(model.wait(()));
    assert!(poll_once(wait.as_mut(), &flag).is_pending());
    // When: software enables interrupt delivery on an otherwise idle channel.
    model
        .write(u32::try_from(IRQ_ENABLE)?, &1_u32.to_le_bytes())
        .await?;
    // Then: the worker is scheduled and the ordinary register value is preserved.
    assert!(flag.0.load(Ordering::SeqCst));
    assert_eq!(poll_once(wait.as_mut(), &flag), Poll::Ready(Ok(())));
    let mut bytes = [0; 4];
    model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn read_wakes_a_parked_worker_and_returns_register_value() -> TestResult {
    // Given: a worker parked on a fresh channel, with no earlier notification.
    let model = Model::new(Device::new((), 1)?);
    let flag = Arc::new(WakeFlag::default());
    let mut wait = pin!(model.wait(()));
    assert!(poll_once(wait.as_mut(), &flag).is_pending());
    let mut bytes = [0xaa; 4];
    // When: software reads the interrupt enable register.
    model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await?;
    // Then: the value is returned and the waiting worker gets a real wakeup.
    assert_eq!(bytes, [0; 4]);
    assert!(flag.0.load(Ordering::SeqCst));
    assert_eq!(poll_once(wait.as_mut(), &flag), Poll::Ready(Ok(())));
    Ok(())
}

#[tokio::test]
async fn read_notification_survives_the_wait_registration_gap() -> TestResult {
    // Given: a fresh channel and a worker that has not yet registered its wait.
    let model = Model::new(Device::new((), 1)?);
    let flag = Arc::new(WakeFlag::default());
    // When: the read commits before the worker begins waiting.
    model.read(u32::try_from(IRQ_ENABLE)?, &mut [0; 4]).await?;
    // Then: one retained notification permits a recheck, not a perpetual ready loop.
    let mut wait = pin!(model.wait(()));
    assert_eq!(poll_once(wait.as_mut(), &flag), Poll::Ready(Ok(())));
    let mut next_wait = pin!(model.wait(()));
    assert!(poll_once(next_wait.as_mut(), &flag).is_pending());
    Ok(())
}

#[tokio::test]
async fn rejected_read_preserves_the_bus_error_and_notifies() -> TestResult {
    // Given: an invalid transaction width and a parked worker.
    let model = Model::new(Device::new((), 1)?);
    let flag = Arc::new(WakeFlag::default());
    let mut wait = pin!(model.wait(()));
    assert!(poll_once(wait.as_mut(), &flag).is_pending());
    let mut bytes = [0xaa; 3];
    // When: hardware rejects the read.
    let result = model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await;
    // Then: transport preserves the result and still finishes its notification boundary.
    assert_eq!(result, Err(Error::Width));
    assert_eq!(bytes, [0xaa; 3]);
    assert!(flag.0.load(Ordering::SeqCst));
    assert_eq!(poll_once(wait.as_mut(), &flag), Poll::Ready(Ok(())));
    Ok(())
}

#[tokio::test]
async fn read_transfers_a_hardware_notification_without_replaying_it() -> TestResult {
    // Given: a real fault notification created beneath the transport.
    let device = Device::new(SharedMemory::new(), 1)?;
    let mut hardware = device.pipeline();
    let mut model = Model::new(device);
    {
        let mut device = model.device.lock().await;
        for offset in [
            IRQ_ENABLE,
            (DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset())
                + CommandRegisters::CAPACITY.offset(),
            (DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset())
                + ResponseRegisters::CAPACITY.offset(),
            (DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::CONTROL.offset())
                + ChannelRegisters::ENABLE.offset(),
            (DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset())
                + CommandRegisters::PRODUCER.offset(),
        ] {
            device.write(offset, &1_u32.to_le_bytes())?;
        }
        let mut operation = Operation::new(());
        let outcome = operation.step(&mut hardware, &mut device);
        drop(device);
        assert_eq!(
            outcome,
            StepOutcome::Complete(Err(Error::Fault(Fault::CommandRead)))
        );
    }
    assert!(!model.irq());
    // When: a read finishes the transport interaction boundary.
    model.read(u32::try_from(IRQ_ENABLE)?, &mut [0; 4]).await?;
    // Then: the emitted notification is transferred once, even though the cause persists.
    assert!(
        hardware_interface_runtime::InterruptSource::claim(&mut model, ())
            .await?
            .is_some()
    );
    model.read(u32::try_from(IRQ_ENABLE)?, &mut [0; 4]).await?;
    assert!(
        hardware_interface_runtime::InterruptSource::claim(&mut model, ())
            .await?
            .is_none()
    );
    Ok(())
}
