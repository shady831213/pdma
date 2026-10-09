use super::*;

#[test]
fn repeated_payload_faults_keep_a_single_faulted_bit() -> TestResult {
    // Given: a payload read remains unavailable across explicit retries.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    run(&mut device)?;
    assert_eq!(fault_status(&mut device)?, 1);
    // When: software requests one retry without repairing the mapping.
    resume(&mut device, 1)?;
    run(&mut device)?;
    // Then: the second fault reports the same single bit and stays parked.
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(device.memory.faults.get(), 2);
    Ok(())
}

#[test]
fn sg_resume_preserves_progress_and_cached_destination_bytes() -> TestResult {
    for write in [false, true] {
        // Given: independently segmented streams suspend after committing three bytes.
        let mut device = setup()?;
        let command = streams(&device)?;
        device
            .memory
            .missing
            .set(Some((if write { 0x1343 } else { 0x1242 }, write)));
        submit(&mut device, command, 0)?;
        run(&mut device)?;
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
            *b"c-----"
        );
        // When: repair is delayed across one explicit retry and source bytes then change.
        resume(&mut device, 1)?;
        run(&mut device)?;
        assert_eq!(fault_status(&mut device)?, 1);
        resume(&mut device, 0)?;
        assert_eq!(fault_status(&mut device)?, 1);
        device.memory.missing.set(None);
        device
            .memory
            .memory
            .write(0x1240.into(), MemoryAlignment::ONE, b"12345")?;
        device
            .memory
            .memory
            .write(0x1300.into(), MemoryAlignment::ONE, b"ZZ")?;
        resume(&mut device, 1)?;
        run(&mut device)?;
        // Then: only remaining bytes commit, and a failed write retains its original read.
        let done = response(&device)?;
        assert_eq!(
            (done.id, done.status, done.bytes_completed()),
            (7, ResponseStatus::Success, 8)
        );
        assert_eq!(
            read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
            1
        );
        assert_eq!(
            read_word(&mut device, CMD + CommandRegisters::CONSUMER.offset())?,
            1
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 2]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"ZZ"
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
            if write { *b"cdefgh" } else { *b"c12345" }
        );
        assert_eq!(fault_status(&mut device)?, 0);
        assert_eq!(device.memory.faults.get(), 2);
    }
    Ok(())
}

#[test]
fn repair_and_zero_resume_do_not_retry_or_admit_later_same_channel_work() -> TestResult {
    // Given: a suspended task followed by a resident same-channel command.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    run(&mut device)?;
    submit(
        &mut device,
        Command {
            id: 99,
            copy_length: 3,
            source: Endpoint::linear(0x1200, 0),
            destination: Endpoint::linear(0x1600, 0),
            ..Command::ZERO
        },
        1,
    )?;
    // When: the backend is repaired without a write-one resume request.
    device.memory.missing.set(None);
    resume(&mut device, 0)?;
    for _ in 0..256 {
        device.step()?;
    }
    // Then: neither the paused task nor its follower performs work until explicitly resumed.
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(
        read_word(&mut device, CMD + CommandRegisters::CONSUMER.offset())?,
        1
    );
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 3]>(0x1600.into(), MemoryAlignment::ONE)?,
        [0; 3]
    );
    assert!(!device.take_interrupt());
    resume(&mut device, 1)?;
    for _ in 0..256 {
        device.step()?;
    }
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
        2
    );
    assert_eq!(response(&device)?.id, 7);
    let raw: ResponseRaw = device.memory.memory.read_typed(
        (0x1100 + u64::try_from(Response::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
    )?;
    assert_eq!(Response::from_bytes(raw.as_ref())?.id, 99);
    Ok(())
}

#[test]
fn mixed_endpoint_resumption_keeps_each_stream_cursor() -> TestResult {
    for linear_source in [false, true] {
        // Given: one SG stream and one linear stream suspend at different boundaries.
        let mut device = setup()?;
        let mut command = streams(&device)?;
        if linear_source {
            device
                .memory
                .memory
                .write(0x1200.into(), MemoryAlignment::ONE, b"abcdefgh")?;
            command.source = Endpoint::linear(0x1200, 0);
            device.memory.missing.set(Some((0x1343, true)));
        } else {
            command.destination = Endpoint::linear(0x1600, 0);
            device.memory.missing.set(Some((0x1242, false)));
        }
        submit(&mut device, command, 0)?;
        run(&mut device)?;
        // When: software repairs the backend and resumes the original task.
        device.memory.missing.set(None);
        resume(&mut device, 1)?;
        run(&mut device)?;
        // Then: one full-length success reports the original task and exact output.
        assert_eq!(
            (response(&device)?.id, response(&device)?.bytes_completed()),
            (7, 8)
        );
        if linear_source {
            assert_eq!(
                device
                    .memory
                    .memory
                    .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
                *b"cdefgh"
            );
        } else {
            assert_eq!(
                device
                    .memory
                    .memory
                    .read_typed::<[u8; 8]>(0x1600.into(), MemoryAlignment::ONE)?,
                *b"abcdefgh"
            );
        }
    }
    Ok(())
}
