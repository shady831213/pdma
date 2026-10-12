mod observed;

use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use observed::Observed;
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, CommandRegisters, Response, ResponseRegisters,
};
use pdma_model_dma::Dma;
use std::cell::Cell;

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Fetched,
    Read,
    PrefixWritten,
    PayloadWritten,
}

fn register(index: usize, offset: usize) -> TestResult<usize> {
    Ok(ChannelId::new(index)
        .register_offset(3)
        .ok_or("channel offset")?
        + offset)
}

fn bytes(memory: &SharedMemory, address: u64) -> TestResult<[u8; 512]> {
    let mut bytes = [0; 512];
    memory.read(
        MemoryAddress::new(address),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    Ok(bytes)
}

fn setup() -> TestResult<Dma<Observed>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(
        Observed {
            memory: memory.clone(),
            payload_read: Cell::new(false),
        },
        3,
    )?;
    for index in 0..3 {
        configure(&mut device, index)?;
        memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    Ok(device)
}

fn reached(device: &mut Dma<Observed>, boundary: Boundary) -> TestResult<bool> {
    let payload = bytes(&device.memory.memory, 0x1400)?;
    Ok(match boundary {
        Boundary::Fetched => {
            word(
                device,
                register(
                    0,
                    ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset(),
                )?,
            )? == 1
        }
        Boundary::Read => device.memory.payload_read.get(),
        Boundary::PrefixWritten => payload.first() == Some(&7),
        Boundary::PayloadWritten => payload == [7; 512],
    })
}

#[test]
fn reset_at_each_interaction_boundary_preserves_only_valid_work() -> TestResult {
    for boundary in [
        Boundary::Fetched,
        Boundary::Read,
        Boundary::PrefixWritten,
        Boundary::PayloadWritten,
    ] {
        for selected in [true, false] {
            // Given: A owns the shared engine and B/C are queued, at an observed interaction boundary.
            let mut device = setup()?;
            let mut hardware = device.command_loop();
            let mut operation = Operation::new(());
            let mut ready = false;
            for _ in 0..128 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
                if reached(&mut device, boundary)? {
                    ready = true;
                    break;
                }
            }
            assert!(ready, "boundary {boundary:?} not reached");
            assert_eq!(
                word(
                    &mut device,
                    register(
                        0,
                        ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
                    )?
                )?,
                0
            );
            let before = bytes(&device.memory.memory, 0x1400)?;
            let reset_index = usize::from(!selected);
            let expected = if selected {
                before
            } else {
                device.memory.memory.write(
                    MemoryAddress::new(0x1200),
                    MemoryAlignment::ONE,
                    &[9; 512],
                )?;
                match boundary {
                    Boundary::Fetched => [9; 512],
                    Boundary::Read | Boundary::PrefixWritten => {
                        let mut expected = [9; 512];
                        expected.get_mut(..256).ok_or("prefix")?.fill(7);
                        expected
                    }
                    Boundary::PayloadWritten => [7; 512],
                }
            };
            // When: hardware completes reset before software reconfigures and submits fresh work.
            let reset = ChannelId::new(reset_index)
                .admin_offset(3)
                .ok_or("admin offset")?;
            device.write(reset, &1_u32.to_le_bytes())?;
            assert_eq!(word(&mut device, reset)?, 1);
            for _ in 0..32 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
                if word(&mut device, reset)? == 0 {
                    break;
                }
            }
            assert_eq!(word(&mut device, reset)?, 0);
            let memory_base = replace(&mut device, reset_index)?;
            for _ in 0..512 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
            }
            // Then: stale work produces no additional writes or response; active peers and fresh work finish exactly once.
            assert_eq!(bytes(&device.memory.memory, 0x1400)?, expected);
            assert_eq!(
                bytes(&device.memory.memory, 0x2400)?,
                if selected { [7; 512] } else { [0; 512] }
            );
            assert_eq!(bytes(&device.memory.memory, 0x3400)?, [7; 512]);
            assert_eq!(bytes(&device.memory.memory, memory_base + 0x800)?, [7; 512]);
            assert_responses(&mut device, reset_index)?;
        }
    }
    Ok(())
}

fn assert_responses(device: &mut Dma<Observed>, reset_index: usize) -> TestResult {
    for index in 0..3 {
        assert_eq!(
            word(
                device,
                register(
                    index,
                    ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
                )?
            )?,
            1
        );
        let raw = device
            .memory
            .memory
            .read_typed::<pdma_model_contract::dma::ResponseRaw>(
                MemoryAddress::new(0x1100 + u64::try_from(index)? * 0x1000),
                MemoryAlignment::ONE,
            )?;
        let response = Response::from_bytes(raw.as_ref())?;
        assert_eq!(
            response.id,
            if index == reset_index {
                99
            } else {
                u64::try_from(index)?
            }
        );
        assert_eq!(response.bytes_completed(), 512);
    }
    Ok(())
}

fn replace(device: &mut Dma<Observed>, reset_index: usize) -> TestResult<u64> {
    configure(device, reset_index)?;
    let memory_base = 0x1000 + u64::try_from(reset_index)? * 0x1000;
    let fresh = pdma_model_contract::dma::Command {
        copy_length: 512,
        sg_context: 0,
        id: 99,
        flags: pdma_model_contract::dma::CompletionFlagsValues { ioc: true },
        source: pdma_model_contract::dma::Endpoint::linear(memory_base + 0x200, 0),
        destination: pdma_model_contract::dma::Endpoint::linear(memory_base + 0x800, 0),
    };
    device.memory.memory.write(
        MemoryAddress::new(memory_base),
        MemoryAlignment::ONE,
        &fresh.to_bytes()?,
    )?;
    device.write(
        register(
            reset_index,
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
        )?,
        &1_u32.to_le_bytes(),
    )?;
    Ok(memory_base)
}
