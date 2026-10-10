use hardware_interface_pipeline::{AfterEach, Repeat, Then};
use pdma_model_dma::{
    CheckRanges, Dma, Intake, NextChunk, Pipeline, ReadCopy, Resettable, UpdateIrq, WriteCopy,
};

#[doc = "Builds the existing single-command pipeline used by the control tests."]
pub fn single_command<M>(device: &Dma<M>) -> Then<Intake, Pipeline> {
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckRanges, Repeat::new(chunk));
    Then::new(
        device.intake(),
        Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            device.responses(),
        ),
    )
}
