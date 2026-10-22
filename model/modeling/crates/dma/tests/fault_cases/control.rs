use super::*;
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::Error;

#[test]
fn rejected_fault_register_accesses_preserve_parked_work() -> TestResult {
    // Given: the command is parked on a missing payload page.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    let status = CTRL + ChannelRegisters::FAULT_STATUS.offset();
    let resume = CTRL + ChannelRegisters::RESUME.offset();
    // When: software violates access permissions, width, or reserved-bit constraints.
    for (offset, bytes, expected) in [
        (status, &[0_u8; 4][..], Error::Access),
        (resume, &[2, 0, 0, 0][..], Error::ReservedBits),
        (resume, &[1, 0, 0, 0, 0, 0, 0, 0][..], Error::Width),
    ] {
        assert_eq!(device.write(offset, bytes), Err(expected));
        run(&mut device)?;
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(device.memory.faults.get(), 1);
    }
    let mut destination = [0xa5; 4];
    assert_eq!(device.read(resume, &mut destination), Err(Error::Access));
    let mut wide_status = [0xa5; 8];
    assert_eq!(device.read(status, &mut wide_status), Err(Error::Width));
    run(&mut device)?;
    // Then: rejected accesses leave caller bytes and the parked command untouched.
    assert_eq!(destination, [0xa5; 4]);
    assert_eq!(wide_status, [0xa5; 8]);
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}

#[test]
fn reset_discards_old_task_and_new_fault_requires_resume() -> TestResult {
    // Given: reset cancels a parked task before the channel is reconfigured.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    let reset = ChannelId::new(0).admin_offset(1).ok_or("reset")?;
    device.write(reset, &1_u32.to_le_bytes())?;
    device.until(|dma| Ok(read_word(dma, reset)? == 0))?;
    assert_eq!(read_word(&mut device, reset)?, 0);
    for (offset, value) in [
        (CMD + CommandRegisters::BASE.offset(), 0x1000_u64),
        (EVENT + EventRegisters::BASE.offset(), 0x1100),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    for offset in [
        CMD + CommandRegisters::CAPACITY.offset(),
        EVENT + EventRegisters::CAPACITY.offset(),
        CTRL + ChannelRegisters::ENABLE.offset(),
    ] {
        device.write(offset, &1_u32.to_le_bytes())?;
    }
    // When: a new task faults after reconfiguration and its mapping is repaired.
    let replacement = Command { id: 91, ..command };
    submit(&mut device, replacement, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    assert_eq!(fault_status(&mut device)?, 1);
    device.memory.missing.set(None);
    for _ in 0..128 {
        device.step()?;
    }
    // Then: only an explicit resume permits the new task to complete.
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    resume(&mut device, 1)?;
    completed(&mut device, replacement.id)?;
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}

#[test]
fn resume_does_not_enable_a_disabled_channel_or_clear_device_fatal() -> TestResult {
    for fatal in [false, true] {
        // Given: a parked task followed by an independent stop condition.
        let mut device = setup()?;
        let command = streams(&device)?;
        device.memory.missing.set(Some((0x1242, false)));
        submit(&mut device, command, 0)?;
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
        assert_eq!(fault_status(&mut device)?, 1);
        if fatal {
            device.signal_fatal();
        } else {
            device.write(
                CTRL + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )?;
        }
        // When: resume is requested after backend repair.
        device.memory.missing.set(None);
        if fatal {
            assert_eq!(
                device.write(
                    CTRL + ChannelRegisters::RESUME.offset(),
                    &1_u32.to_le_bytes()
                ),
                Err(pdma_model_dma::Error::Halted)
            );
        } else {
            resume(&mut device, 1)?;
        }
        for _ in 0..128 {
            device.step()?;
        }
        // Then: resume cannot bypass the independent stop condition.
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
        if !fatal {
            assert_eq!(
                read_word(&mut device, CTRL + ChannelRegisters::ENABLE.offset())?,
                0
            );
            device.write(
                CTRL + ChannelRegisters::ENABLE.offset(),
                &1_u32.to_le_bytes(),
            )?;
            completed(&mut device, command.id)?;
            assert_eq!(
                read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
                0
            );
        }
    }
    Ok(())
}

#[test]
fn resume_without_a_fault_does_not_prearm_later_faults() -> TestResult {
    for request in [0, 1] {
        // Given: software writes resume while no command is faulted.
        let mut device = setup()?;
        let command = streams(&device)?;
        resume(&mut device, request)?;
        device.memory.missing.set(Some((0x1242, false)));
        // When: the next command encounters a payload fault.
        submit(&mut device, command, 0)?;
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
        device.write(
            CTRL + ChannelRegisters::IRQ_MASK.offset(),
            &0_u32.to_le_bytes(),
        )?;
        run(&mut device)?;
        // Then: the request was consumed and cannot retry or bypass that fault.
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(device.memory.faults.get(), 1);
        assert_eq!(
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            0
        );
    }
    Ok(())
}
