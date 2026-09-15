use super::{Model, TestResult};
use hardware_interface_pipeline::{AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{InterruptSource, RegisterAccess};
use pdma_model_contract::{DeviceControl, DeviceRegisters};
use pdma_model_dma::{Device, ResetControl};

#[tokio::test]
async fn emitted_notification_survives_device_reset_and_is_claimed_once() -> TestResult {
    // Given: the platform has accepted a hardware error notification.
    let mut device = Device::new((), 1)?;
    device.write(
        DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    device.signal_fatal();
    let mut model = Model::new(device);
    assert!(model.irq());
    // When: software resets the device before the interrupt dispatcher claims it.
    model
        .write(
            u32::try_from(DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    let outcome = model
        .interact(|device| {
            assert!(matches!(
                Operation::new(()).step(&mut ResetControl, device),
                StepOutcome::Progressed
            ));
            StepOutcome::<(), std::convert::Infallible, ()>::Complete(Ok(()))
        })
        .await?;
    assert!(matches!(outcome, StepOutcome::Complete(Ok(()))));
    // Then: reset cannot retract an already emitted notification or cause repeat delivery.
    assert!(model.claim(()).await?.is_some());
    assert!(model.claim(()).await?.is_none());
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn enabling_after_masked_fatal_does_not_emit_a_notification() -> TestResult {
    // Given: a fatal event occurred with device notifications disabled.
    let mut device = Device::new((), 1)?;
    device.signal_fatal();
    let mut model = Model::new(device);
    // When: software enables notifications after the event.
    model
        .write(
            u32::try_from(DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    // Then: the fault remains readable but no historical notification is replayed.
    let mut bytes = [0; 4];
    model
        .read(
            u32::try_from(DeviceRegisters::CONTROL.offset() + DeviceControl::FAULT.offset())?,
            &mut bytes,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert!(model.claim(()).await?.is_none());
    Ok(())
}
