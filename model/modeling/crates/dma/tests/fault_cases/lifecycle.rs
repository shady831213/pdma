use super::*;
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};

#[test]
fn pending_fault_response_is_discarded_by_channel_reset() -> TestResult {
    // Given: hardware observed a payload fault but has not yet published its response.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    device
        .memory
        .memory
        .write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    device.write(
        CMD + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut pipeline = device.command_loop();
    let mut operation = Operation::new(());
    for _ in 0..128 {
        let _step = operation.step(&mut pipeline, &mut device);
        if device.memory.faults.get() != 0 {
            break;
        }
    }
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    // When: reset arrives before the terminal publication.
    let reset = ChannelId::new(0).admin_offset(1).ok_or("reset address")?;
    device.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..128 {
        let _step = operation.step(&mut pipeline, &mut device);
        if read_word(&mut device, reset)? == 0 {
            break;
        }
    }
    // Then: reset waits for retirement, discards the response and preserves committed writes.
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
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
fn unrelated_work_completes_while_faulting_page_remains_absent() -> TestResult {
    // Given: an unconsumed PageFault response and an absent source page.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    assert_eq!(
        submit(&mut device, command, 0)?.status,
        ResponseStatus::PageFault
    );
    assert!(device.take_interrupt());
    // When: a later independent command uses resident storage on the same channel.
    let peer = Command {
        id: 99,
        source: Endpoint::linear(0x1200, 0),
        destination: Endpoint::linear(0x1600, 0),
        copy_length: 3,
        ..Command::ZERO
    };
    let response = submit(&mut device, peer, 1)?;
    // Then: progress does not depend on page repair or draining the earlier response.
    assert_eq!(
        response,
        Response {
            id: 99,
            info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 3 }),
            ..Response::ZERO
        }
    );
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 3]>(0x1600.into(), MemoryAlignment::ONE)?,
        *b"abc"
    );
    assert!(!device.take_interrupt());
    assert_eq!(device.memory.faults.get(), 1);
    Ok(())
}

#[test]
fn mixed_endpoints_use_their_own_continuation_representation() -> TestResult {
    for linear_source in [true, false] {
        // Given: only one endpoint is SG, with different boundaries from its linear peer.
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
        // When: the noninitial chunk faults and software uses the returned positions directly.
        let response = submit(&mut device, command, 0)?;
        assert_eq!(response.status, ResponseStatus::PageFault);
        let (completed, source, destination) = if linear_source {
            (2, Position::linear(2), Position::scatter_gather(1, 0))
        } else {
            (3, Position::scatter_gather(1, 0), Position::linear(3))
        };
        assert_eq!(
            (
                response.bytes_completed(),
                response
                    .page_fault()
                    .ok_or("expected PageFault")?
                    .source_next(),
                response
                    .page_fault()
                    .ok_or("expected PageFault")?
                    .destination_next()
            ),
            (completed, source, destination)
        );
        consume(&mut device, 0)?;
        device.memory.missing.set(None);
        command.source = command
            .source
            .with_start(
                response
                    .page_fault()
                    .ok_or("expected PageFault")?
                    .source_next(),
            )
            .ok_or("position mode mismatch")?;
        command.destination = command
            .destination
            .with_start(
                response
                    .page_fault()
                    .ok_or("expected PageFault")?
                    .destination_next(),
            )
            .ok_or("position mode mismatch")?;
        command.copy_length -= response.bytes_completed();
        let done = submit(&mut device, command, 1)?;
        // Then: only the remaining range completes and its output matches the original stream.
        assert_eq!(done.status, ResponseStatus::Success);
        assert_eq!(done.bytes_completed(), 8 - completed);
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
