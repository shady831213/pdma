use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{ChannelId, CommandRegisters};
use pdma_model_dma::{Candidate, Dma};

#[test]
fn completed_candidates_leave_every_descriptor_unaccepted() -> TestResult {
    // Given: two ready channels sharing one execution engine.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 2)?;
    for index in 0..2 {
        configure(&mut device, index)?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    // When: independent input stages finish producing their candidate values.
    for index in 0..2 {
        let channel = ChannelId::new(index);
        assert_eq!(
            Operation::new(()).step(&mut Candidate::new(channel), &mut device)?,
            StepOutcome::Complete(Ok(Some(channel)))
        );
    }
    // Then: value production neither pops descriptors nor starts shared execution.
    for index in 0..2 {
        let base = ChannelId::new(index).register_offset(2).ok_or("channel")?;
        assert_eq!(
            word(&mut device, base + CommandRegisters::CONSUMER.offset())?,
            0
        );
    }
    Ok(())
}
