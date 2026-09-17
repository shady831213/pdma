use super::{acquire, support::*};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SoftwareCpu};
use pdma_model_contract::{
    ChannelBlock, ChannelRegisters, ControlQueue, ControlRequest, ControlRequestOpcode,
    ControlRingState, DeviceRegisters, WINDOW_CONTROL_BASE,
};
use pdma_model_software::ring::SoftRingError;

#[tokio::test]
async fn misaligned_bootstrap_descriptor_is_rejected() -> TestResult {
    // Given: an otherwise valid descriptor is copied to a misaligned bootstrap address.
    let (bus, memory) = fixture()?;
    let mut bytes = vec![0; pdma_model_contract::ControlQueue::RECORD_SIZE];
    memory
        .read(MemoryAddress::new(QUEUE), MemoryAlignment::ONE, &mut bytes)
        .unwrap();
    memory
        .write(MemoryAddress::new(QUEUE + 1), MemoryAlignment::ONE, &bytes)
        .unwrap();
    bus.registers.lock().unwrap().insert(
        register(pdma_model_contract::ControlRegisters::BOOTSTRAP.offset()).unwrap(),
        (QUEUE + 1).to_le_bytes().to_vec(),
    );
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory)));
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
    let result = SoftwareCpu::new()
        .run_main(pdma_model_firmware::main(&bus, memory.clone()))
        .await;
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
    let result = SoftwareCpu::new()
        .run_main(pdma_model_firmware::main(&bus, memory))
        .await;
    // Then: a valid distance does not authorize Host to initialize firmware progress.
    assert!(matches!(result, Err(pdma_model_firmware::Error::Ring(_))));
    Ok(())
}

#[tokio::test]
async fn release_response_waits_for_real_reset_completion() -> TestResult {
    // Given: acquisition succeeded and Host submitted release after freeing response space.
    let (bus, memory) = fixture()?;
    publish(&memory, acquire(1), 1)?;
    let cpu = SoftwareCpu::new();
    let mut program = std::pin::pin!(cpu.run_main(pdma_model_firmware::main(&bus, memory.clone())));
    assert!(poll(program.as_mut()).is_pending());
    write_word(
        &memory,
        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
        1,
    )?;
    publish(
        &memory,
        ControlRequest {
            id: 2,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0,
        },
        2,
    )?;
    assert!(poll(program.as_mut()).is_pending());
    let reset = u32::try_from(
        DeviceRegisters::CHANNELS_OFFSET
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
    )
    .unwrap();
    assert_eq!(
        bus.registers.lock().unwrap().get(&reset),
        Some(&1_u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        bus.registers.lock().unwrap().get(&WINDOW_CONTROL_BASE),
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
