use super::{
    Channel, ChannelConfig, ChannelIrqMaskValues, ChannelRegisters, DmaFaultCode, Model,
    RegisterAccess, SoftwareCpu, fixture,
};
use hardware_interface_runtime::{CpuAccessError, CpuContextError};
use pdma_model_contract::dma::{ChannelBlock, ChannelId};
use pdma_model_host::DmaError;

#[tokio::test]
async fn control_operations_can_run_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a driver created on one CPU over an unrestricted register transport.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let first = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let channel = first
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // When: another CPU invokes the same driver's control operations.
    other
        .run_main(channel.set_irq_mask(ChannelIrqMaskValues::new(3)))
        .await?;
    other.run_main(channel.set_dma_irq_enabled(true)).await?;
    let cause = other.run_main(channel.irq_cause()).await?;
    let fault = other.run_main(channel.dma_fault()).await?;
    // Then: register effects and returned hardware state do not depend on the setup CPU.
    assert_eq!(cause.raw(), 0);
    assert_eq!(fault, DmaFaultCode::None);
    let mut mask = [0; 4];
    model
        .read(
            u32::try_from(
                ChannelBlock::ALIGNMENT
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::IRQ_MASK.offset(),
            )?,
            &mut mask,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(mask), 3);
    Ok(())
}

#[tokio::test]
async fn cpu_specific_bus_still_enforces_its_own_context() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: assembly deliberately supplies a CPU-specific register capability.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let owner = SoftwareCpu::new();
    let channel = owner
        .run_main(Channel::setup(
            owner.bind_registers(model.clone()),
            memory,
            ChannelConfig {
                id: ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // When: another CPU calls the ordinary driver through that capability.
    let result = SoftwareCpu::new()
        .run_main(channel.set_irq_mask(ChannelIrqMaskValues::new(3)))
        .await;
    // Then: the bus rejects access before the register changes.
    assert!(matches!(
        result,
        Err(DmaError::Bus(CpuAccessError::Cpu(
            CpuContextError::Mismatch
        )))
    ));
    let mut mask = [0; 4];
    model
        .read(
            u32::try_from(
                ChannelBlock::ALIGNMENT
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::IRQ_MASK.offset(),
            )?,
            &mut mask,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(mask), 0);
    Ok(())
}
