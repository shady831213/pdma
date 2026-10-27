use pdma_model_contract::dma::{
    CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint,
};

use super::*;

#[test]
fn repeated_payload_faults_keep_a_single_faulted_bit() -> TestResult {
    // Given: a payload read remains unavailable across explicit retries.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    assert_eq!(fault_status(&mut device)?, 1);
    // When: software requests one retry without repairing the mapping.
    resume(&mut device, 1)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
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
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(
            device
                .memory
                .memory
                .read_buffer::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
            *b"c-----"
        );
        // When: repair is delayed across one explicit retry and source bytes then change.
        resume(&mut device, 1)?;
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
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
        completed(&mut device, command.id())?;
        // Then: only remaining bytes commit, and a failed write retains its original read.

        assert_eq!(
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            0
        );
        assert_eq!(
            read_word(&mut device, CMD + CommandRegisters::CONSUMER.offset())?,
            2
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_buffer::<[u8; 2]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"ZZ"
        );
        assert_eq!(
            device
                .memory
                .memory
                .read_buffer::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
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
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    let follower = {
        let original = Command::ZERO;
        let pdma_model_contract::dma::Command::Copy(copy) = original else {
            return Err("copy fixture required".into());
        };
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 99,
            copy_length: 3,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1200,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1600,
                offset: 0,
            }),
            ..copy
        })
    };
    submit(&mut device, follower, 1)?;
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
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_buffer::<[u8; 3]>(0x1600.into(), MemoryAlignment::ONE)?,
        [0; 3]
    );
    assert!(!device.take_interrupt());
    resume(&mut device, 1)?;
    completed(&mut device, command.id())?;
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_buffer::<[u8; 3]>(0x1600.into(), MemoryAlignment::ONE)?,
        *b"abc"
    );
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
            crate::support::copy_mut(&mut command)?.source =
                CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: 0x1200,
                    offset: 0,
                });
            device.memory.missing.set(Some((0x1343, true)));
        } else {
            crate::support::copy_mut(&mut command)?.destination =
                CopyCommandDestinationValues::Linear(LinearEndpoint {
                    address: 0x1600,
                    offset: 0,
                });
            device.memory.missing.set(Some((0x1242, false)));
        }
        submit(&mut device, command, 0)?;
        device.until(|dma| Ok(fault_status(dma)? == 1))?;
        // When: software repairs the backend and resumes the original task.
        device.memory.missing.set(None);
        resume(&mut device, 1)?;
        completed(&mut device, command.id())?;
        // Then: one full-length success reports the original task and exact output.

        if linear_source {
            assert_eq!(
                device
                    .memory
                    .memory
                    .read_buffer::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
                *b"cdefgh"
            );
        } else {
            assert_eq!(
                device
                    .memory
                    .memory
                    .read_buffer::<[u8; 8]>(0x1600.into(), MemoryAlignment::ONE)?,
                *b"abcdefgh"
            );
        }
    }
    Ok(())
}
