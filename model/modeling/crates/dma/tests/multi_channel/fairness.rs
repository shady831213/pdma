use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRecovery, ChannelRegisters, CopyCommandDestinationValues,
    CopyCommandSourceValues, EventRegisters, LinearEndpoint,
};

use super::support::{TestResult, completion, configure, submit, word};
use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use hardware_interface_pipeline::StepOutcome;
use pdma_model_dma::Dma;

#[test]
fn stopped_or_fatal_channel_does_not_stall_peers() -> TestResult {
    for fatal in [false, true] {
        // Given: A has a command-fetch fatal or a stopping error; B/C have fresh work.
        let memory = SharedMemory::new();
        let (start, length) = if fatal {
            (0x2000, 0x2000)
        } else {
            (0x1000, 0x3000)
        };
        memory.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(start),
            length,
            MemoryAlignment::ONE,
        )?])?;
        let mut device = Dma::new(memory, 3)?;
        configure(&mut device, 0)?;
        let base = ChannelId::new(0)
            .register_offset(3)
            .ok_or("channel offset")?;
        if fatal {
            device.write(
                base + ChannelBlock::COMMAND.offset()
                    + pdma_model_contract::dma::CommandRegisters::PRODUCER.offset(),
                &1_u32.to_le_bytes(),
            )?;
        } else {
            stop_channel(&mut device, base)?;
        }
        for index in 1..3 {
            configure(&mut device, index)?;
            submit(&mut device, index, u64::try_from(index)?)?;
        }
        // When: the sustained graph visits all channels while A remains stopped.
        let mut hardware = device.hardware();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..512 {
            assert!(matches!(
                operation.step(&mut hardware, &mut device)?,
                StepOutcome::InProgress
            ));
        }
        // Then: both peer Fences complete, and A's stopping condition stays local.
        for index in 1..3 {
            assert_eq!(
                word(
                    &mut device,
                    ChannelId::new(index).register_offset(3).ok_or("offset")?
                        + ChannelBlock::EVENT.offset()
                        + EventRegisters::PRODUCER.offset()
                )?,
                0
            );
            assert_eq!(completion(&device, index)?, u64::try_from(index)? + 1);
        }
        if fatal {
            assert_eq!(
                word(
                    &mut device,
                    base + ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::RECOVERY.offset()
                        + ChannelRecovery::FATAL.offset()
                )?,
                1
            );
        } else {
            assert_eq!(
                word(
                    &mut device,
                    base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset()
                )?,
                1
            );
        }
    }
    Ok(())
}

fn stop_channel(device: &mut Dma<SharedMemory>, base: usize) -> TestResult {
    let command = pdma_model_contract::dma::Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 1,
        copy_length: 4,
        sg_context: 0,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0x1400,
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    device.memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    device.write(
        base + ChannelBlock::COMMAND.offset()
            + pdma_model_contract::dma::CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut hardware = device.hardware();
    let mut operation = hardware_interface_pipeline::Operation::new(());
    let mut finished = false;
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, device)? {
            result?;
            if word(
                device,
                base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset(),
            )? == 1
            {
                finished = true;
                break;
            }
            operation = hardware_interface_pipeline::Operation::new(());
        }
    }
    assert!(finished);
    Ok(())
}
