use super::*;
use pdma_model_contract::dma::ChannelInterrupts;
use pdma_model_contract::dma::ChannelRecovery;

#[test]
fn linear_fault_is_silent_under_every_notification_mask() -> TestResult {
    for write in [false, true] {
        for mask in 0..4_u32 {
            for trap in [false, true] {
                // Given: a transfer faults inside its second scratch block.
                let mut device = setup()?;
                device
                    .memory
                    .memory
                    .write(0x1400.into(), MemoryAlignment::ONE, &[0xa5; 600])?;
                let command = {
                    let original = Command::ZERO;
                    let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) =
                        original.payload
                    else {
                        return Err("copy fixture required".into());
                    };
                    Command::copy(
                        9,
                        pdma_model_contract::dma::CopyCommand {
                            source: Endpoint::linear(0x1400, 7),
                            destination: Endpoint::linear(0x1800, 11),
                            copy_length: 512,
                            ..copy
                        },
                    )
                };
                device.write(
                    CTRL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::MASK.offset(),
                    &mask.to_le_bytes(),
                )?;
                device
                    .memory
                    .missing
                    .set(Some((if write { 0x1915 } else { 0x1510 }, write)));
                // When: the second payload access faults.
                submit(&mut device, command, 0)?;
                run(&mut device)?;
                // Then: committed bytes remain, with no event or IRQ regardless of notification masks.
                let mut bytes = [0; 512];
                device
                    .memory
                    .memory
                    .read(0x180b.into(), MemoryAlignment::ONE, &mut bytes)?;
                assert_eq!(&bytes[..256], &[0xa5; 256]);
                assert_eq!(&bytes[256..], &[0; 256]);
                assert!(!device.take_interrupt());
                assert_eq!(
                    read_word(
                        &mut device,
                        CTRL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
                    )?,
                    0
                );
                assert_eq!(
                    read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
                    0
                );
                assert_eq!(fault_status(&mut device)?, 1);
                device.memory.missing.set(None);
                resume(&mut device, 1)?;
                run(&mut device)?;
                device
                    .memory
                    .memory
                    .read(0x180b.into(), MemoryAlignment::ONE, &mut bytes)?;
                assert_eq!(bytes, [0xa5; 512]);
                if trap {
                    submit(&mut device, Command::trap(10), 1)?;
                    run(&mut device)?;
                }
                assert_eq!(device.take_interrupt(), trap && mask & 2 == 0);
                assert_eq!(
                    read_word(
                        &mut device,
                        CTRL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
                    )?,
                    if trap { 2 } else { 0 }
                );
            }
        }
    }
    Ok(())
}

#[test]
fn metadata_and_event_faults_keep_stopping_and_fatal_boundaries() -> TestResult {
    for (address, write) in [(0x1000, false), (0x1400, false), (0x1100, true)] {
        // Given: a missing command, SG metadata, or event page rather than payload.
        let mut device = setup()?;
        let mut command = streams(&device)?;
        if write {
            crate::support::copy_mut(&mut command)?.source = Endpoint::linear(0x3000, 0);
        }
        device.memory.missing.set(Some((address, write)));
        submit(&mut device, command, 0)?;
        // When: the same hardware pipeline encounters the missing page.
        run(&mut device)?;
        // Then: only SG failure publishes a stopping event; ring failures are fatal.
        let expected_fault = if address == 0x1000 {
            1
        } else if write {
            2
        } else {
            0
        };
        assert_eq!(
            read_word(
                &mut device,
                CTRL + ChannelRegisters::RECOVERY.offset() + ChannelRecovery::FATAL.offset()
            )?,
            expected_fault
        );
        if address == 0x1400 {
            let event = event(&device)?;
            assert_eq!(event.code, EventCode::ListReadError);
            assert_eq!(event.bytes_completed(), 0);
            assert_eq!(
                read_word(
                    &mut device,
                    CTRL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
                )?,
                5
            );
        } else {
            assert_eq!(
                read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
                0
            );
        }
        assert!(device.take_interrupt());
    }
    Ok(())
}
