use super::{acquire, support::*};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlRequestOpcode, ControlRingState,
};
use pdma_model_software::ring::SoftRingError;

#[tokio::test]
async fn misaligned_bootstrap_descriptor_is_rejected() -> TestResult {
    // Given: an otherwise valid descriptor is copied to a misaligned bootstrap address.
    let (bus, memory) = fixture()?;
    let mut bytes = vec![0; pdma_model_contract::control::ControlQueue::RECORD_SIZE];
    memory
        .read(MemoryAddress::new(QUEUE), MemoryAlignment::ONE, &mut bytes)
        .unwrap();
    memory
        .write(MemoryAddress::new(QUEUE + 1), MemoryAlignment::ONE, &bytes)
        .unwrap();
    bus.registers.lock().unwrap().insert(
        register(
            pdma_model_contract::hw_components::control::ControlRegisters::CONFIG.offset()
                + pdma_model_contract::hw_components::control::ControlConfiguration::BOOTSTRAP
                    .offset(),
        )
        .unwrap(),
        (QUEUE + 1).to_le_bytes().to_vec(),
    );
    let mut program = std::pin::pin!(run(&bus, memory));
    // When: firmware attaches through that bootstrap address.
    let result = poll(program.as_mut());
    // Then: attachment rejects the schema alignment violation.
    assert!(matches!(
        result,
        std::task::Poll::Ready(Err(pdma_model_firmware::Error::Ring(_)))
    ));
    Ok(())
}

#[tokio::test]
async fn malformed_request_remains_unconsumed() -> TestResult {
    // Given: a published request contains an undefined opcode.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(12), 1)?;
    memory
        .write(
            MemoryAddress::new(COMMAND + u64::try_from(ControlRequest::OPCODE_OFFSET).unwrap()),
            MemoryAlignment::ONE,
            &u32::MAX.to_le_bytes(),
        )
        .unwrap();
    // When: firmware decodes that request.
    let result = run(&bus, memory.clone()).await;
    // Then: decoding fails before ownership or command consumption changes.
    assert!(matches!(
        result,
        Err(pdma_model_firmware::Error::SoftRing(SoftRingError::Codec(
            _
        )))
    ));
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        0
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn attachment_rejects_nonzero_firmware_owned_cursor() -> TestResult {
    // Given: Host supplies a descriptor with a forged consumed-command cursor.
    let (bus, memory) = fixture()?;
    write_word(
        &memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::PRODUCER_OFFSET,
        1,
    )?;
    write_word(
        &memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    // When: firmware attaches.
    let result = run(&bus, memory).await;
    // Then: a valid distance does not authorize Host to initialize firmware progress.
    assert!(matches!(result, Err(pdma_model_firmware::Error::Ring(_))));
    Ok(())
}

#[tokio::test]
async fn release_response_waits_for_real_reset_completion() -> TestResult {
    // Given: acquisition succeeded and Host submitted release after freeing response space.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
    write_word(
        &memory,
        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    publish(
        &memory,
        ControlRequest {
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
            id: 2,
            function: 0,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0,
        },
        2,
    )?;
    assert!(poll(program.as_mut()).is_pending());
    let reset = u32::try_from(
        pdma_model_contract::dma::ChannelId::new(0)
            .admin_offset(1)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&ADDRESSES.windows),
        Some(&0_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    assert!(poll(program.as_mut()).is_pending());
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        1
    );
    // When: the hardware reset register actually clears.
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    assert!(poll(program.as_mut()).is_pending());
    // Then: release acknowledges only after that completion.
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET
        )?,
        2
    );
    assert_eq!(response(&memory)?.id, 2);
    Ok(())
}

#[tokio::test]
async fn release_response_backpressure_preserves_queued_acquisition() -> TestResult {
    // Given: a release is resetting while another acquisition is queued.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let mut program = std::pin::pin!(run(&bus, memory.clone()));
    assert!(poll(program.as_mut()).is_pending());
    let consumer = ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET;
    let producer = ControlQueue::RESPONSE_OFFSET + ControlRingState::PRODUCER_OFFSET;
    write_word(&memory, consumer, 1)?;
    publish(
        &memory,
        ControlRequest {
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
            id: 2,
            function: 0,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0,
        },
        2,
    )?;
    assert!(poll(program.as_mut()).is_pending());
    publish(&memory, acquire(3), 3)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(read_word(&memory, producer)?, 1);
    let reset = u32::try_from(
        pdma_model_contract::dma::ChannelId::new(0)
            .admin_offset(1)
            .ok_or("admin offset")?,
    )?;
    // When: hardware finishes reset and its response fills the ring.
    bus.registers
        .lock()
        .unwrap()
        .insert(reset, 0_u32.to_le_bytes().to_vec());
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    // Then: the queued request waits for response space and cannot overwrite the release response.
    assert_eq!(
        (read_word(&memory, producer)?, response(&memory)?.id),
        (2, 2)
    );
    assert_eq!(
        read_word(
            &memory,
            ControlQueue::COMMAND_OFFSET + ControlRingState::CONSUMER_OFFSET
        )?,
        2
    );
    let writes = bus.writes.lock().unwrap().len();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(bus.writes.lock().unwrap().len(), writes);
    write_word(&memory, consumer, 2)?;
    bus.notification.raise();
    for _ in 0..4 {
        assert!(poll(program.as_mut()).is_pending());
    }
    assert_eq!(
        (read_word(&memory, producer)?, response(&memory)?.id),
        (3, 3)
    );
    assert_eq!(
        response(&memory)?.status,
        pdma_model_contract::control::ControlResponseStatus::Success
    );
    Ok(())
}
