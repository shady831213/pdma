use super::*;

#[test]
fn linear_fault_is_attempt_relative_and_obeys_normal_notification_masks() -> TestResult {
    for write in [false, true] {
        for mask in 0..4_u32 {
            for ioc in [false, true] {
                // Given: a transfer longer than a scratch block and a fault inside its second block.
                let mut device = setup()?;
                device
                    .memory
                    .memory
                    .write(0x1400.into(), MemoryAlignment::ONE, &[0xa5; 600])?;
                let command = Command {
                    id: 9,
                    source: Endpoint::linear(0x1400, 7),
                    destination: Endpoint::linear(0x1800, 11),
                    copy_length: 512,
                    flags: CompletionFlagsValues { ioc },
                    ..Command::ZERO
                };
                device.write(
                    CTRL + ChannelRegisters::IRQ_MASK.offset(),
                    &mask.to_le_bytes(),
                )?;
                let missing = if write { 0x1915 } else { 0x1510 };
                device.memory.missing.set(Some((missing, write)));
                // When: the second access faults after one committed block.
                let response = submit(&mut device, command, 0)?;
                // Then: progress and both starts point to the first uncommitted byte, not the faulting byte.
                assert_eq!(response.status, ResponseStatus::PageFault);
                assert_eq!(response.bytes_completed(), 256);
                assert_eq!(
                    response
                        .page_fault()
                        .ok_or("expected PageFault")?
                        .fault_address,
                    missing
                );
                assert_eq!(
                    response
                        .page_fault()
                        .ok_or("expected PageFault")?
                        .source_next(),
                    Position::linear(263)
                );
                assert_eq!(
                    response
                        .page_fault()
                        .ok_or("expected PageFault")?
                        .destination_next(),
                    Position::linear(267)
                );
                let mut bytes = [0; 512];
                device
                    .memory
                    .memory
                    .read(0x180b.into(), MemoryAlignment::ONE, &mut bytes)?;
                assert_eq!(&bytes[..256], &[0xa5; 256]);
                assert_eq!(&bytes[256..], &[0; 256]);
                assert_eq!(
                    device.take_interrupt(),
                    mask & 1 == 0 || (ioc && mask & 2 == 0)
                );
                assert_eq!(
                    read_word(&mut device, CTRL + ChannelRegisters::IRQ_CAUSE.offset())?,
                    1 | if ioc { 2 } else { 0 }
                );
            }
        }
    }
    Ok(())
}

#[test]
fn missing_metadata_keeps_existing_command_and_response_fault_boundaries() -> TestResult {
    for (address, write) in [(0x1000, false), (0x1400, false), (0x1100, true)] {
        // Given: a missing command, SG metadata, or response page rather than payload.
        let mut device = setup()?;
        let command = streams(&device)?;
        device.memory.missing.set(Some((address, write)));
        device
            .memory
            .memory
            .write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
        device.write(
            CMD + CommandRegisters::PRODUCER.offset(),
            &1_u32.to_le_bytes(),
        )?;
        // When: the same hardware pipeline encounters the missing page.
        run(&mut device)?;
        // Then: only the SG failure publishes a stopping response; ring failures are fatal.
        let expected_fault = if address == 0x1000 {
            1
        } else if write {
            2
        } else {
            0
        };
        assert_eq!(
            read_word(&mut device, CTRL + ChannelRegisters::FATAL.offset())?,
            expected_fault
        );
        if address == 0x1400 {
            let raw = device
                .memory
                .memory
                .read_typed::<ResponseRaw>(0x1100.into(), MemoryAlignment::ONE)?;
            let response = Response::from_bytes(raw.as_ref())?;
            assert_eq!(response.status, ResponseStatus::ListReadError);
            assert_eq!(response.bytes_completed(), 0);
            assert_eq!(
                read_word(&mut device, CTRL + ChannelRegisters::IRQ_CAUSE.offset())?,
                5
            );
        } else {
            assert_eq!(
                read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
                0
            );
        }
        assert!(device.take_interrupt());
    }
    Ok(())
}
