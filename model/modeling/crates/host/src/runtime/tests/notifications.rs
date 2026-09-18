use super::{Model, TestResult};
use hardware_interface_pipeline::{AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{InterruptSource, RegisterAccess};
use pdma_model_contract::DmaControl;
use pdma_model_dma::{Dma, ResetControl};

#[tokio::test]
async fn emitted_notification_survives_device_reset_and_is_claimed_once() -> TestResult {
    // Given: the platform has accepted a hardware error notification.
    let mut device = Dma::new((), 1)?;
    device.write(
        (pdma_model_contract::DmaDiscovery::EXTENT
            + pdma_model_contract::DmaAdmin::CONTROL.offset())
            + DmaControl::IRQ_ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    device.signal_fatal();
    let mut model = Model::new(device);
    assert!(model.irq());
    // When: software resets the device before the interrupt dispatcher claims it.
    model
        .write(
            u32::try_from(
                (pdma_model_contract::DmaDiscovery::EXTENT
                    + pdma_model_contract::DmaAdmin::CONTROL.offset())
                    + DmaControl::RESET.offset(),
            )?,
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
    let mut device = Dma::new((), 1)?;
    device.signal_fatal();
    let mut model = Model::new(device);
    // When: software enables notifications after the event.
    model
        .write(
            u32::try_from(
                (pdma_model_contract::DmaDiscovery::EXTENT
                    + pdma_model_contract::DmaAdmin::CONTROL.offset())
                    + DmaControl::IRQ_ENABLE.offset(),
            )?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    // Then: the fault remains readable but no historical notification is replayed.
    let mut bytes = [0; 4];
    model
        .read(
            u32::try_from(
                (pdma_model_contract::DmaDiscovery::EXTENT
                    + pdma_model_contract::DmaAdmin::CONTROL.offset())
                    + DmaControl::FAULT.offset(),
            )?,
            &mut bytes,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert!(model.claim(()).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn masking_preserves_emitted_notification_across_model_clones() -> TestResult {
    // Given: a fatal notification emitted before the platform claims it.
    let mut device = Dma::new((), 1)?;
    let irq_enable = (pdma_model_contract::DmaDiscovery::EXTENT
        + pdma_model_contract::DmaAdmin::CONTROL.offset())
        + DmaControl::IRQ_ENABLE.offset();
    device.write(irq_enable, &1_u32.to_le_bytes())?;
    device.signal_fatal();
    let mut first = Model::new(device);
    let mut second = first.clone();
    // When: software masks the device through one transport handle.
    first
        .write(u32::try_from(irq_enable)?, &0_u32.to_le_bytes())
        .await?;
    // Then: both handles retain readiness, but their claims consume one notification.
    first.wait_ready().await?;
    second.wait_ready().await?;
    assert!(first.claim(()).await?.is_some());
    assert!(second.claim(()).await?.is_none());
    assert!(!second.irq());
    Ok(())
}
