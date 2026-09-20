use super::support::{
    Fixture, TestResult, WINDOWS, advance, bytes, command, configure, publish, response, setup,
};
use hardware_interface_pipeline::{Operation, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{ChannelId, Endpoint, ResponseStatus, WindowControl};

#[test]
fn reset_before_regrant_prevents_old_work_from_entering_the_new_space() -> TestResult {
    // Given: old work is paused after its first payload write on channel zero.
    let Fixture {
        mut dma,
        spaces: [old, new],
        windows,
    } = setup()?;
    old.write_typed(
        MemoryAddress::new(0x1600),
        MemoryAlignment::ONE,
        &[0x6d; 1024],
    )?;
    new.write_typed(
        MemoryAddress::new(0x1b00),
        MemoryAlignment::ONE,
        &[0xa5; 1024],
    )?;
    let mut work = command(1);
    work.source = Endpoint::linear(0x1600, 1024);
    work.destination = Endpoint::linear(0x1b00, 1024);
    publish(&mut dma, &old, (ChannelId::new(0), work))?;
    let mut pipeline = dma.command_loop();
    let mut operation = Operation::new(());
    for _ in 0..128 {
        if bytes::<1>(&old, 0x1b00)? != [0] {
            break;
        }
        let _outcome = StepExecutor::new()
            .step(&mut pipeline, &mut operation, &mut dma)
            .map_err(|error| format!("{error:?}"))?;
    }
    let before = bytes::<1024>(&old, 0x1b00)?;
    assert!(before.contains(&0x6d) && before.contains(&0));
    // When: management waits for reset completion before installing the new owner.
    windows
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .management_write(WINDOWS, &0_u32.to_le_bytes())?;
    let reset = ChannelId::new(0).admin_offset(2).ok_or("reset address")?;
    dma.write(reset, &1_u32.to_le_bytes())?;
    advance(&mut dma, &mut pipeline, &mut operation)?;
    let mut busy = [0; 4];
    dma.read(reset, &mut busy)?;
    assert_eq!(busy, [0; 4]);
    windows
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .management_write(
            WINDOWS + WindowControl::OWNER.offset(),
            &1_u32.to_le_bytes(),
        )?;
    new.write_typed(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        b"new-work",
    )?;
    configure(&mut dma, ChannelId::new(0))?;
    publish(&mut dma, &new, (ChannelId::new(0), command(2)))?;
    advance(&mut dma, &mut pipeline, &mut operation)?;
    // Then: new work completes in the new space, without late old payload or response writes.
    assert_eq!(bytes::<1024>(&old, 0x1b00)?, before);
    assert_eq!(bytes::<1024>(&new, 0x1b00)?, [0xa5; 1024]);
    assert_eq!(bytes::<8>(&new, 0x1300)?, *b"new-work");
    assert_eq!(bytes::<8>(&old, 0x1100)?, [0; 8]);
    assert_eq!(response(&new)?.id, 2);
    assert_eq!(response(&new)?.status, ResponseStatus::Success);
    Ok(())
}
