use super::*;
use pdma_model_contract::dma::{ChannelId, ContextId};
use pdma_model_dma::MaintenanceError;

#[test]
fn payload_fault_does_not_publish_an_event() -> TestResult {
    // Given: an accepted command with a missing source payload page.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    // When: the shared engine reaches the page fault.
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    // Then: the command remains retained without completion or interrupt.
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert!(!device.take_interrupt());
    assert_eq!(fault_status(&mut device)?, 1);
    Ok(())
}

#[test]
fn reset_discards_parked_work_and_clears_fault_status() -> TestResult {
    // Given: suspended work with a committed prefix.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    let reset = ChannelId::new(0).admin_offset(1).ok_or("reset address")?;
    // When: reset is driven without any resume request.
    device.write(reset, &1_u32.to_le_bytes())?;
    device.until(|dma| Ok(read_word(dma, reset)? == 0))?;
    // Then: cleanup finishes, committed bytes stay, and the old task cannot revive.
    assert_eq!(read_word(&mut device, reset)?, 0);
    resume(&mut device, 1)?;
    assert_eq!(fault_status(&mut device)?, 0);
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"c-----"
    );
    assert!(!device.take_interrupt());
    Ok(())
}

#[test]
fn drain_reports_parked_channel_and_abort_discards_without_completion() -> TestResult {
    for abort in [false, true] {
        // Given: a parked command holds a non-ring context.
        let mut device = setup()?;
        let mut command = streams(&device)?;
        crate::support::copy_mut(&mut command)?.source_context = 7;
        device.memory.missing.set(Some((0x1242, false)));
        submit(&mut device, command, 0)?;
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
        let context = ContextId::new(crate::support::copy(&command)?.source_context);
        let channel = ChannelId::new(0);
        // When: management requests maintenance of the retained reference.
        if abort {
            device.abort_context(context, &[channel])?;
        } else {
            device.drain_context(context, &[channel])?;
        }
        device.until(|dma| Ok(dma.context_status(context) != Ok(false)))?;
        // Then: Drain fails explicitly; Abort retires parked work without publishing.
        if abort {
            assert_eq!(device.context_status(context), Ok(true));
            assert_eq!(fault_status(&mut device)?, 0);
        } else {
            assert_eq!(
                device.context_status(context),
                Err(MaintenanceError::ChannelStopped(channel))
            );
            assert_eq!(
                device.failed_context_channels(context).collect::<Vec<_>>(),
                vec![channel]
            );
            assert_eq!(fault_status(&mut device)?, 1);
        }
        assert_eq!(
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            0
        );
        assert!(!device.take_interrupt());
    }
    Ok(())
}

#[test]
fn abort_of_running_task_discards_it_only_when_payload_fault_parks_it() -> TestResult {
    // Given: a command has been accepted before Abort arrives.
    let mut device = setup()?;
    let mut command = streams(&device)?;
    crate::support::copy_mut(&mut command)?.destination_context = 7;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(read_word(dma, CMD + CommandRegisters::CONSUMER.offset())? == 1))?;
    assert_eq!(device.memory.faults.get(), 0);
    // When: Abort waits for the running command and that command encounters a page fault.
    let context = ContextId::new(crate::support::copy(&command)?.destination_context);
    device.abort_context(context, &[ChannelId::new(0)])?;
    device.until(|dma| Ok(dma.context_status(context) != Ok(false)))?;
    // Then: committed bytes remain and maintenance completes without requiring resume.
    assert_eq!(device.context_status(context), Ok(true));
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(fault_status(&mut device)?, 0);
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"c-----"
    );
    assert!(!device.take_interrupt());
    Ok(())
}
