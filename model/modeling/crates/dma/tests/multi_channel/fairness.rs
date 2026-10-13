use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::StepOutcome;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, ResponseRegisters};
use pdma_model_dma::Dma;

#[test]
fn full_response_or_fatal_channel_does_not_stall_peers() -> TestResult {
    for fatal in [false, true] {
        // Given: A is either faulted or has a full response ring; B/C have fresh work.
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
            fill_response_ring(&mut device, base)?;
        }
        for index in 1..3 {
            configure(&mut device, index)?;
            submit(&mut device, index, u64::try_from(index)?)?;
        }
        // When: the sustained graph visits all channels while A cannot publish.
        let mut hardware = device.service();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..512 {
            assert!(matches!(
                operation.step(&mut hardware, &mut device)?,
                StepOutcome::InProgress
            ));
        }
        // Then: both peers complete, and A's fault or full ring stays local.
        for index in 1..3 {
            assert_eq!(
                word(
                    &mut device,
                    ChannelId::new(index).register_offset(3).ok_or("offset")?
                        + ChannelBlock::RESPONSE.offset()
                        + ResponseRegisters::PRODUCER.offset()
                )?,
                1
            );
        }
        if fatal {
            assert_eq!(
                word(
                    &mut device,
                    base + ChannelBlock::CONTROL.offset() + ChannelRegisters::FATAL.offset()
                )?,
                1
            );
        } else {
            assert_eq!(
                word(
                    &mut device,
                    base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
                )?,
                2
            );
        }
    }
    Ok(())
}

fn fill_response_ring(device: &mut Dma<SharedMemory>, base: usize) -> TestResult {
    let command = pdma_model_contract::dma::Command {
        copy_length: 0,
        sg_context: 0,
        id: 1,
        flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
        source: pdma_model_contract::dma::Endpoint::linear(0x1200, 0),
        destination: pdma_model_contract::dma::Endpoint::linear(0x1400, 0),
    };
    for producer in 1..=3_u32 {
        device.memory.write(
            MemoryAddress::new(
                0x1000
                    + u64::from((producer - 1) % 2)
                        * u64::try_from(pdma_model_contract::dma::Command::RECORD_SIZE)?,
            ),
            MemoryAlignment::ONE,
            &command.to_bytes()?,
        )?;
        device.write(
            base + ChannelBlock::COMMAND.offset()
                + pdma_model_contract::dma::CommandRegisters::PRODUCER.offset(),
            &producer.to_le_bytes(),
        )?;
        if producer == 3 {
            break;
        }
        let mut hardware = device.service();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        let mut finished = false;
        for _ in 0..128 {
            assert!(matches!(
                operation.step(&mut hardware, device)?,
                StepOutcome::InProgress
            ));
            if word(
                device,
                base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset(),
            )? == producer
            {
                finished = true;
                break;
            }
        }
        assert!(finished);
    }
    Ok(())
}
