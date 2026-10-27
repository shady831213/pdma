use super::support::{TestResult, configure, submit, word};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_pipeline::StepOutcome;
use pdma_model_contract::dma::{ChannelBlock, ChannelId, CommandRegisters, EventRegisters};
use pdma_model_dma::Dma;

#[test]
fn arbiter_accepts_no_peer_command_until_selected_command_finishes() -> TestResult {
    // Given: three queued commands and one shared execution unit.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory.clone(), 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    let mut hardware = device.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let mut first_write = false;
    // When: A reaches its first committed chunk, before event publication.
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        ));
        if memory.read_buffer::<[u8; 1]>(MemoryAddress::new(0x1400), MemoryAlignment::ONE)? == [7] {
            first_write = true;
            break;
        }
    }
    assert!(first_write);
    assert_eq!(
        word(
            &mut device,
            ChannelId::new(0).register_offset(3).ok_or("offset")?
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );
    // Then: B/C remain in their command rings and have made no payload writes.
    for index in 1..3 {
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(index).register_offset(3).ok_or("offset")?
                    + ChannelBlock::COMMAND.offset()
                    + CommandRegisters::CONSUMER.offset()
            )?,
            0
        );
        let mut bytes = [0; 512];
        memory.read(
            MemoryAddress::new(0x1400 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &mut bytes,
        )?;
        assert_eq!(bytes, [0; 512]);
    }
    for _ in 0..256 {
        for index in 1..3 {
            assert_eq!(
                word(
                    &mut device,
                    ChannelId::new(index).register_offset(3).ok_or("offset")?
                        + ChannelBlock::COMMAND.offset()
                        + CommandRegisters::CONSUMER.offset()
                )?,
                0
            );
        }
        if device.command_request(ChannelId::new(0))?.is_some() {
            let mut bytes = [0; 512];
            memory.read(MemoryAddress::new(0x1400), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(bytes, [7; 512]);
            return Ok(());
        }
        assert_eq!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        );
    }
    Err("selected command did not complete before watchdog".into())
}
