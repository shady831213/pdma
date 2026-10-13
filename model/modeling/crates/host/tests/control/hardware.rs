use hardware_interface_pipeline::{AfterEach, Then};
use pdma_model_dma::{
    CheckRanges, Dma, Fetch, NextChunk, Pipeline, ReadCopy, Resettable, UpdateIrq, WriteCopy,
};

/// Builds one finite chunk for the control boundary tests.
pub fn single_chunk<M>(device: &Dma<M>) -> Then<Fetch, Pipeline> {
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckRanges, chunk);
    Then::new(
        Fetch,
        Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            device.responses(),
        ),
    )
}
