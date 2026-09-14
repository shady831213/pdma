use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::{ChannelBlock, ChannelId, CommandRegisters, ResponseRegisters};
use pdma_model_device::Device;

#[test]
fn arbiter_accepts_no_peer_command_until_selected_command_finishes() -> TestResult {
    // Given: three queued commands and one shared execution unit.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x3000,
            MemoryAlignment::ONE,
        )?])?
        .try_install(&memory)?;
    let mut device = Device::new(memory.clone(), 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    let mut hardware = device.command_loop();
    let mut operation = Operation::new(());
    let mut first_write = false;
    // When: A reaches its first committed chunk, before response publication.
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut hardware, &mut device),
            StepOutcome::Progressed
        ));
        if memory.read_typed::<[u8; 1]>(MemoryAddress::new(0x1400), MemoryAlignment::ONE)? == [7] {
            first_write = true;
            break;
        }
    }
    assert!(first_write);
    assert_eq!(
        word(
            &mut device,
            ChannelId::new(0).register_offset().ok_or("offset")?
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    // Then: B/C remain in their command rings and have made no payload writes.
    for index in 1..3 {
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(index).register_offset().ok_or("offset")?
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
    Ok(())
}
