use super::*;
use hardware_interface_pipeline::StepOutcome;
use pdma_model_contract::dma::ChannelId;

pub(super) fn pair() -> TestResult<(Dma<Paged>, usize, Command)> {
    let mut device = Dma::new(setup()?.dma.memory, 2)?;
    let first = streams(&device)?;
    let peer = {
        let original = Command::ZERO;
        let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) = original.payload else {
            return Err("copy fixture required".into());
        };
        Command::copy(
            99,
            pdma_model_contract::dma::CopyCommand {
                copy_length: 3,
                source: Endpoint::linear(0x1200, 0),
                destination: Endpoint::linear(0x1600, 0),
                ..copy
            },
        )
    };
    let mut peer_event = 0;
    for (index, command, command_base, event_base) in [
        (0, first, 0x1000_u64, 0x1100_u64),
        (1, peer, 0x1700, 0x1800),
    ] {
        let base = ChannelId::new(index)
            .register_offset(2)
            .ok_or("channel base")?;
        let event = base + ChannelBlock::EVENT.offset();
        let control = base + ChannelBlock::CONTROL.offset();
        for (offset, value) in [
            (base + CommandRegisters::BASE.offset(), command_base),
            (event + EventRegisters::BASE.offset(), event_base),
        ] {
            device.write(offset, &value.to_le_bytes())?;
        }
        device.write(
            base + CommandRegisters::CAPACITY.offset(),
            &2_u32.to_le_bytes(),
        )?;
        for offset in [
            event + EventRegisters::CAPACITY.offset(),
            control + ChannelRegisters::ENABLE.offset(),
        ] {
            device.write(offset, &1_u32.to_le_bytes())?;
        }
        device.memory.memory.write(
            command_base.into(),
            MemoryAlignment::ONE,
            &command.to_bytes()?,
        )?;
        let fence = Command::fence(
            command.id,
            pdma_model_contract::dma::WordWrite {
                target: pdma_model_contract::dma::WordTarget {
                    address: 0x1fe0 + u64::try_from(index)? * 8,
                    context: 0,
                    width: pdma_model_contract::dma::WordTargetWidth::Bits64,
                },
                value: command.id,
            },
        );
        device.memory.memory.write(
            (command_base + u64::try_from(Command::RECORD_SIZE)?).into(),
            MemoryAlignment::ONE,
            &fence.to_bytes()?,
        )?;
        device.write(
            base + CommandRegisters::PRODUCER.offset(),
            &2_u32.to_le_bytes(),
        )?;
        if index == 1 {
            peer_event = event;
        }
    }
    device.memory.missing.set(Some((0x1242, false)));
    Ok((device, peer_event, peer))
}

#[test]
fn peer_channel_completes_while_faulted_channel_stays_parked() -> TestResult {
    // Given: two physical channels with independent rings and one missing source page.
    let (mut device, peer_event, peer) = pair()?;
    let mut pipeline = device.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    // When: the shared engine runs both channels with the page still absent.
    for _ in 0..512 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut device)?,
            StepOutcome::InProgress
        ));
        if u64::from_le_bytes(
            device
                .memory
                .memory
                .read_typed::<[u8; 8]>(0x1fe8.into(), MemoryAlignment::ONE)?,
        ) == 99
        {
            break;
        }
    }
    // Then: peer work completes once and the faulted task is neither retried nor published.
    assert_eq!(
        read_word(&mut device, peer_event + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 3]>(0x1600.into(), MemoryAlignment::ONE)?,
        *b"abc"
    );
    assert_eq!(
        u64::from_le_bytes(
            device
                .memory
                .memory
                .read_typed::<[u8; 8]>(0x1fe8.into(), MemoryAlignment::ONE)?
        ),
        peer.id
    );
    Ok(())
}

#[test]
fn parked_reset_finishes_while_an_accepted_peer_is_disabled() -> TestResult {
    // Given: one parked task and an accepted peer that is disabled before payload access.
    let (mut device, peer_event, _) = pair()?;
    let peer = ChannelId::new(1).register_offset(2).ok_or("peer")?;
    let enable = peer + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset();
    let reset = ChannelId::new(0).admin_offset(2).ok_or("reset")?;
    let mut pipeline = device.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut device)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut device, peer + CommandRegisters::CONSUMER.offset())? == 1 {
            break;
        }
    }
    assert_eq!(fault_status(&mut device)?, 1);
    assert_eq!(
        read_word(&mut device, peer + CommandRegisters::CONSUMER.offset())?,
        1
    );
    device.write(enable, &0_u32.to_le_bytes())?;
    // When: only the parked channel is reset while the shared engine remains blocked.
    device.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut device)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut device, reset)? == 0 {
            break;
        }
    }
    // Then: parked cleanup completes independently and the peer retains its accepted task.
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert_eq!(fault_status(&mut device)?, 0);
    assert_eq!(
        read_word(&mut device, peer_event + EventRegisters::PRODUCER.offset())?,
        0
    );
    device.write(enable, &1_u32.to_le_bytes())?;
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut device)?,
            StepOutcome::InProgress
        ));
        if u64::from_le_bytes(
            device
                .memory
                .memory
                .read_typed::<[u8; 8]>(0x1fe8.into(), MemoryAlignment::ONE)?,
        ) == 99
        {
            break;
        }
    }
    assert_eq!(
        read_word(&mut device, peer_event + EventRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(device.memory.faults.get(), 1);
    assert_eq!(
        u64::from_le_bytes(
            device
                .memory
                .memory
                .read_typed::<[u8; 8]>(0x1fe8.into(), MemoryAlignment::ONE)?
        ),
        99
    );
    Ok(())
}
