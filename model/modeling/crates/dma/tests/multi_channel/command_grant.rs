use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters};
use pdma_model_dma::Dma;

#[test]
fn sustained_loop_reselects_if_grant_loses_eligibility_before_fetch() -> TestResult {
    // Given: two ready channels, with A selected but not fetched.
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
    let [first, peer] = [0, 1].map(|index| ChannelId::new(index).register_offset(2));
    let first = first.ok_or("first channel")?;
    let peer = peer.ok_or("peer channel")?;
    let consumer = CommandRegisters::CONSUMER.offset();
    let mut engine = device.command_loop();
    let mut operation = Operation::new(());
    assert!(matches!(
        operation.step(&mut engine, &mut device),
        StepOutcome::Progressed
    ));
    assert_eq!(word(&mut device, first + consumer)?, 0);
    assert_eq!(word(&mut device, peer + consumer)?, 0);
    // When: A becomes disabled before its grant accepts a descriptor.
    device.write(
        first + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
        &0_u32.to_le_bytes(),
    )?;
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut engine, &mut device),
            StepOutcome::Progressed | StepOutcome::Pending(())
        ));
        if word(&mut device, peer + consumer)? == 1 {
            break;
        }
    }
    // Then: B is accepted and A is still queued.
    assert_eq!(word(&mut device, peer + consumer)?, 1);
    assert_eq!(word(&mut device, first + consumer)?, 0);
    Ok(())
}
