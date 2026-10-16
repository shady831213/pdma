use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::StepOutcome;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, Response, ResponseRegisters,
    ResponseStatus,
};
use pdma_model_dma::Dma;

#[test]
fn sustained_loop_reselects_if_disabled_before_fetch() -> TestResult {
    reselect_before_fetch(false)
}

#[test]
fn sustained_loop_reselects_if_reset_before_fetch() -> TestResult {
    reselect_before_fetch(true)
}

fn reselect_before_fetch(reset: bool) -> TestResult {
    for prefix in 0..128 {
        // Given: every pre-acceptance boundary, including the selected-but-unfetched grant.
        let memory = SharedMemory::new();
        memory.add_regions(&[MemoryRegion::try_new(
            0x1000.into(),
            0x3000,
            MemoryAlignment::ONE,
        )?])?;
        let mut device = Dma::new(memory, 3)?;
        for index in 0..3 {
            configure(&mut device, index)?;
            submit(&mut device, index, u64::try_from(index)?)?;
        }
        let [first, peer, third] = [0, 1, 2].map(|index| ChannelId::new(index).register_offset(3));
        let first = first.ok_or("first channel")?;
        let peer = peer.ok_or("peer channel")?;
        let third = third.ok_or("third channel")?;
        let consumer = ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset();
        let mut engine = device.hardware();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..prefix {
            assert!(matches!(
                operation.step(&mut engine, &mut device)?,
                StepOutcome::InProgress
            ));
        }
        if word(&mut device, first + consumer)? == 1 {
            return Ok(());
        }
        assert_eq!(word(&mut device, first + consumer)?, 0);
        assert_eq!(word(&mut device, peer + consumer)?, 0);
        // When: A loses eligibility before acceptance.
        let (register, value) = if reset {
            (
                ChannelId::new(0).admin_offset(3).ok_or("reset register")?,
                1_u32,
            )
        } else {
            (
                first + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
                0,
            )
        };
        device.write(register, &value.to_le_bytes())?;
        for _ in 0..128 {
            assert!(matches!(
                operation.step(&mut engine, &mut device)?,
                StepOutcome::InProgress
            ));
            if word(&mut device, peer + consumer)? == 1 {
                break;
            }
        }
        // Then: B is accepted without accepting work from A or C.
        assert_eq!(word(&mut device, peer + consumer)?, 1);
        assert_eq!(word(&mut device, first + consumer)?, 0);
        assert_eq!(word(&mut device, third + consumer)?, 0);
    }
    Err("command acceptance exceeded interaction bound".into())
}

#[test]
fn reset_and_reconfigure_before_acceptance_uses_the_new_command_generation() -> TestResult {
    for prefix in 0..128 {
        // Given: each boundary before first acceptance, including selection before Fetch.
        let memory = SharedMemory::new();
        memory.add_regions(&[MemoryRegion::try_new(
            0x1000.into(),
            0x3000,
            MemoryAlignment::ONE,
        )?])?;
        let mut device = Dma::new(memory, 3)?;
        for index in 0..3 {
            configure(&mut device, index)?;
            submit(&mut device, index, u64::try_from(index)?)?;
        }
        let consumer = ChannelId::new(0).register_offset(3).ok_or("offset")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::CONSUMER.offset();
        let mut hardware = device.hardware();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..prefix {
            assert!(matches!(
                operation.step(&mut hardware, &mut device)?,
                StepOutcome::InProgress
            ));
        }
        if word(&mut device, consumer)? == 1 {
            return Ok(());
        }
        assert_eq!(word(&mut device, consumer)?, 0);
        // When: software resets A and publishes a replacement command before acceptance.
        device.write(
            ChannelId::new(0).admin_offset(3).ok_or("offset")?,
            &1_u32.to_le_bytes(),
        )?;
        let reset = ChannelId::new(0).admin_offset(3).ok_or("offset")?;
        for _ in 0..64 {
            if word(&mut device, reset)? == 0 {
                break;
            }
            let _ = operation.step(&mut hardware, &mut device)?;
        }
        assert_eq!(word(&mut device, reset)?, 0);
        configure(&mut device, 0)?;
        submit(&mut device, 0, 99)?;
        let producer = ChannelId::new(0).register_offset(3).ok_or("offset")?
            + ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::PRODUCER.offset();
        for _ in 0..256 {
            assert!(matches!(
                operation.step(&mut hardware, &mut device)?,
                StepOutcome::InProgress
            ));
            if word(&mut device, producer)? == 1 {
                break;
            }
        }
        // Then: the replacement task completes under its fresh generation, not as stale work.
        assert_eq!(word(&mut device, producer)?, 1);
        let raw = device
            .memory
            .read_typed::<pdma_model_contract::dma::ResponseRaw>(
                MemoryAddress::new(0x1100),
                MemoryAlignment::ONE,
            )?;
        let response = Response::from_bytes(raw.as_ref())?;
        assert_eq!(response.id, 99);
        assert_eq!(response.status, ResponseStatus::Success);
        assert_eq!(response.bytes_completed(), 512);
        assert_eq!(word(&mut device, consumer)?, 1);
        for index in 1..3 {
            let base = ChannelId::new(index).register_offset(3).ok_or("offset")?;
            let producer =
                base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset();
            for _ in 0..256 {
                if word(&mut device, producer)? == 1 {
                    break;
                }
                let _ = operation.step(&mut hardware, &mut device)?;
            }
            assert_eq!(
                word(&mut device, producer)?,
                1,
                "prefix {prefix}, peer {index}"
            );
            assert_eq!(
                word(
                    &mut device,
                    base + ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset()
                )?,
                1
            );
            let raw = device
                .memory
                .read_typed::<pdma_model_contract::dma::ResponseRaw>(
                    MemoryAddress::new(0x1100 + u64::try_from(index)? * 0x1000),
                    MemoryAlignment::ONE,
                )?;
            let response = Response::from_bytes(raw.as_ref())?;
            assert_eq!(response.id, u64::try_from(index)?);
            assert_eq!(response.status, ResponseStatus::Success);
            assert_eq!(response.bytes_completed(), 512);
        }
    }
    Err("command acceptance exceeded interaction bound".into())
}
