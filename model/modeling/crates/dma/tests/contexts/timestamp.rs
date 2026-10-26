use super::*;
use pdma_model_contract::dma::{CommandHeader, Opcode};

#[test]
fn timestamp_uses_target_context_not_ring_or_default_context() -> TestResult {
    use pdma_model_contract::dma::{Event, EventCode, TimestampCommand};
    #[derive(Debug)]
    struct ExternalTimer(u64);
    impl pdma_model_dma::Timer for ExternalTimer {
        fn sample(&self) -> u64 {
            self.0
        }
    }
    // Given: identical virtual addresses in independent spaces; metadata uses context four.
    let sample = 0x1234_5678_9abc_def0_u64;
    for target in [1, 7] {
        let mut dma = Dma::with_sideband(Spaces::new()?, 1, |access| access.context.raw())?
            .with_timer(std::sync::Arc::new(ExternalTimer(sample)));
        let command = Command::Timestamp(TimestampCommand {
            header: CommandHeader {
                opcode: Opcode::Timestamp,
            },
            id: 7,
            address: 0x1300,
            context: target,
        });
        dma.memory.write_with_sideband(
            &4,
            0x1000.into(),
            MemoryAlignment::ONE,
            &command.to_bytes()?,
        )?;
        let base = ChannelId::new(0).register_offset(1).ok_or("layout")?;
        dma.write(
            base + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::RING_CONTEXT.offset(),
            &4_u32.to_le_bytes(),
        )?;
        for (offset, address) in [
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                0x1000_u64,
            ),
            (
                ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                0x1100,
            ),
        ] {
            dma.write(base + offset, &address.to_le_bytes())?;
        }
        for offset in [
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
        ] {
            dma.write(base + offset, &1_u32.to_le_bytes())?;
        }
        let mut hardware = dma.hardware();
        let mut driver = hardware_interface_pipeline::HardwareDriver::new();
        // When: Timestamp writes using the command's selector, or that selector is rejected.
        for _ in 0..128 {
            let _ = driver.step(&mut hardware, &mut dma)?;
        }
        // Then: no fallback to ring/default context can hide an invalid target selector.
        for context in 0..5 {
            let mut bytes = [0; 8];
            dma.memory.read_with_sideband(
                &context,
                0x1300.into(),
                MemoryAlignment::ONE,
                &mut bytes,
            )?;
            assert_eq!(
                u64::from_le_bytes(bytes),
                if context == target { sample } else { 0 }
            );
        }
        let mut producer = [0; 4];
        dma.read(
            base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset(),
            &mut producer,
        )?;
        assert_eq!(u32::from_le_bytes(producer), u32::from(target == 7));
        if target == 7 {
            let mut bytes = vec![0; Event::RECORD_SIZE];
            dma.memory
                .read_with_sideband(&4, 0x1100.into(), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(Event::from_bytes(&bytes)?.code, EventCode::WriteError);
        }
    }
    Ok(())
}
