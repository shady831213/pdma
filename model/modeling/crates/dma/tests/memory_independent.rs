//! Hardware control stages do not require a memory backend.

use hardware_interface_pipeline::{Operation, StepOutcome};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, DmaAdmin, DmaControl, DmaDiscovery,
};
use pdma_model_dma::{Dma, ResetControl, UpdateIrq};

#[test]
fn reset_and_irq_control_require_no_memory_backend() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a DMA instance with no memory capability and a pending reset.
    let mut device = Dma::with_sideband((), 1, std::convert::identity)?;
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    // When: independently composed control stages advance on the full device context.
    assert_eq!(
        Operation::new(()).step(&mut ResetControl, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    assert_eq!(
        Operation::new(()).step(&mut UpdateIrq, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    // Then: idle reset completes and the control-only device remains quiescent.
    let mut bytes = [0; 4];
    device.read(reset, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    let channel = ChannelId::new(0)
        .register_offset(1)
        .ok_or("channel offset")?
        + ChannelBlock::CONTROL.offset();
    for offset in [
        DmaDiscovery::EXTENT + DmaAdmin::CONTROL.offset() + DmaControl::FATAL.offset(),
        channel + ChannelRegisters::FATAL.offset(),
        channel + ChannelRegisters::IRQ_CAUSE.offset(),
    ] {
        device.read(offset, &mut bytes)?;
        assert_eq!(u32::from_le_bytes(bytes), 0);
    }
    assert!(!device.irq());
    Ok(())
}
