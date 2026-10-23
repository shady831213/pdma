//! Reset isolation while an opaque execution stage retains command state.
use pdma_model_contract::dma::ChannelConfiguration;
/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{HardwareDriver, Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, CommandRegisters, DmaAdmin, DmaDiscovery, EventRegisters,
};
use pdma_model_dma::{ChunkInput, Dma, Error, ExecutionContext};
use std::{cell::Cell, rc::Rc};
use support::{TestResult, command, submit};

struct Stalled(Rc<Cell<bool>>, Option<pdma_model_dma::Flow<ChunkInput>>);
impl<M> Stage<ExecutionContext<'_, M>> for Stalled {
    type Input = pdma_model_dma::Flow<ChunkInput>;
    type Output = pdma_model_dma::Flow<pdma_model_dma::Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.1.is_none() {
            self.1 = input.take();
        }
        let Some(flow) = self.1.as_ref() else {
            return Ok(StepOutcome::InProgress);
        };
        self.0.set(true);
        Ok(match flow.lifecycle(context) {
            Ok(()) => StepOutcome::InProgress,
            Err(error) => StepOutcome::Complete(Ok(self
                .1
                .take()
                .ok_or(hardware_interface_pipeline::RunError::Exhausted)?
                .stop(error))),
        })
    }
}

#[test]
fn unrelated_idle_reset_completes_without_stepping_stalled_pipeline() -> TestResult {
    // Given: channel zero owns an opaque, indefinitely stalled execution stage.
    let (_, memory) = support::setup(0x1100, 2)?;
    let mut dma = Dma::new(memory.clone(), 2)?;
    let base = ChannelBlock::ALIGNMENT;
    for (offset, value) in [
        (
            base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
            0x1100,
        ),
    ] {
        dma.write(offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            base + ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            base + ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            2,
        ),
        (
            base + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            1,
        ),
    ] {
        dma.write(offset, &value.to_le_bytes())?;
    }
    submit(&mut dma, &memory, command())?;
    let entered = Rc::new(Cell::new(false));
    let mut hardware = dma.hardware_with(Stalled(Rc::clone(&entered), None));
    let mut driver = HardwareDriver::new();
    for _ in 0..128 {
        let _ = driver.step(&mut hardware, &mut dma)?;
        if entered.get() {
            break;
        }
    }
    assert!(entered.get());
    // When: software resets the unrelated idle channel, with no hardware step.
    let reset = DmaDiscovery::EXTENT
        + DmaAdmin::CHANNELS_OFFSET
        + pdma_model_contract::dma::ChannelAdmin::EXTENT;
    dma.write(reset, &1_u32.to_le_bytes())?;
    // Then: its real cleanup is already complete despite the stalled peer.
    assert_eq!(support::read_word(&mut dma, reset)?, 0);
    let device_reset = DmaDiscovery::EXTENT
        + DmaAdmin::CONTROL.offset()
        + pdma_model_contract::dma::DmaControl::RESET.offset();
    dma.write(device_reset, &1_u32.to_le_bytes())?;
    assert_eq!(support::read_word(&mut dma, device_reset)?, 1);
    let peer_base = pdma_model_contract::dma::ChannelId::new(1)
        .register_offset(2)
        .ok_or("peer layout")?;
    assert_eq!(
        dma.write(
            peer_base + CommandRegisters::BASE.offset(),
            &0x1000_u64.to_le_bytes()
        ),
        Err(Error::ResetInProgress)
    );
    for _ in 0..128 {
        let _ = driver.step(&mut hardware, &mut dma)?;
        if support::read_word(&mut dma, device_reset)? == 0 {
            break;
        }
    }
    assert_eq!(support::read_word(&mut dma, device_reset)?, 0);
    dma.write(
        peer_base + CommandRegisters::BASE.offset(),
        &0x1000_u64.to_le_bytes(),
    )?;
    Ok(())
}
