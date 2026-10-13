//! Single-command stage assembly for host boundary tests.

use hardware_interface_pipeline::{AfterEach, Repeat, Then};
use pdma_model_dma::{
    CheckRanges, Dma, Fetch, NextChunk, Pipeline, ReadCopy, Resettable, UpdateIrq, WriteCopy,
};

/// Assemble one command for host boundary tests without starting a device loop.
pub fn single_command<M>(device: &Dma<M>) -> Then<Fetch, Pipeline> {
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckRanges, Repeat::new(chunk));
    Then::new(
        Fetch,
        Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            device.responses(),
        ),
    )
}
