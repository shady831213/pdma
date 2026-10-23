use super::Model;
use pdma_model_contract::dma::ChannelConfiguration;
use pdma_model_contract::dma::ChannelInterrupts;
mod cooperative;
mod notifications;
use hardware_interface_pipeline::{AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use pdma_model_contract::dma::{ChannelRegisters, CommandRegisters, EventRegisters};
use pdma_model_dma::{ChannelFatal, Dma, Error};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const IRQ_ENABLE: usize = (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset())
    + ChannelRegisters::IRQ.offset()
    + ChannelInterrupts::ENABLE.offset();

#[tokio::test]
async fn write_preserves_register_value() -> TestResult {
    // Given: an idle DMA channel.
    let model = Model::new(Dma::new((), 1)?);
    // When: software enables interrupt delivery on an otherwise idle channel.
    model
        .write(u32::try_from(IRQ_ENABLE)?, &1_u32.to_le_bytes())
        .await?;
    // Then: the ordinary register value is preserved without fabricating an IRQ.
    let mut bytes = [0; 4];
    model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn read_returns_register_value() -> TestResult {
    // Given: a fresh channel.
    let model = Model::new(Dma::new((), 1)?);
    let mut bytes = [0xaa; 4];
    // When: software reads the interrupt enable register.
    model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await?;
    // Then: the value is returned without creating a hardware notification.
    assert_eq!(bytes, [0; 4]);
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn software_write_is_visible_at_the_next_hardware_boundary() -> TestResult {
    // Given: a fresh channel before the next hardware interaction.
    let model = Model::new(Dma::new((), 1)?);
    // When: software commits a write before that interaction.
    let enabled = 1_u32.to_le_bytes();
    model.write(u32::try_from(IRQ_ENABLE)?, &enabled).await?;
    // Then: hardware observes the committed register value on its next step.
    let outcome = model
        .interact(|device| {
            let mut bytes = [0; 4];
            Ok(StepOutcome::Complete(
                device.read(IRQ_ENABLE, &mut bytes).map(|()| bytes),
            ))
        })
        .await?;
    assert_eq!(outcome, Ok(StepOutcome::Complete(Ok(enabled))));
    Ok(())
}

#[tokio::test]
async fn rejected_read_preserves_the_bus_error_and_output() -> TestResult {
    // Given: an invalid transaction width.
    let model = Model::new(Dma::new((), 1)?);
    let mut bytes = [0xaa; 3];
    // When: hardware rejects the read.
    let result = model.read(u32::try_from(IRQ_ENABLE)?, &mut bytes).await;
    // Then: transport preserves the error and leaves the caller's buffer untouched.
    assert_eq!(result, Err(Error::Width));
    assert_eq!(bytes, [0xaa; 3]);
    Ok(())
}

#[tokio::test]
async fn read_transfers_a_hardware_notification_without_replaying_it() -> TestResult {
    // Given: a real fault notification created beneath the transport.
    let device = Dma::new(SharedMemory::new(), 1)?;
    let mut hardware = pdma_model_dma::Fetch;
    let mut model = Model::new(device);
    let outcome = {
        let mut device = model.device.lock().await;
        for offset in [
            IRQ_ENABLE,
            (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset())
                + CommandRegisters::CAPACITY.offset(),
            (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset())
                + EventRegisters::CAPACITY.offset(),
            (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset())
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset())
                + CommandRegisters::PRODUCER.offset(),
        ] {
            device.write(offset, &1_u32.to_le_bytes())?;
        }
        let id = pdma_model_contract::dma::ChannelId::new(0);
        let Some(pdma_model_dma::CommandRequest::Fetch(snapshot)) = device.command_request(id)?
        else {
            return Err("no command request".into());
        };
        let mut operation = Operation::new(Some(pdma_model_dma::CommandRequest::Fetch(snapshot)));
        let mut outcome = None;
        for _ in 0..256 {
            if let StepOutcome::Complete(Ok(flow)) = operation.step(&mut hardware, &mut *device)? {
                outcome = Some(
                    Operation::new(vec![(0, flow.finish())])
                        .step(&mut pdma_model_dma::Retire, &mut *device),
                );
                break;
            }
        }
        drop(device);
        outcome
    };
    assert!(matches!(
        outcome,
        Some(Ok(StepOutcome::Complete(Err(Error::ChannelFatal(
            ChannelFatal::CommandRead
        )))))
    ));
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
