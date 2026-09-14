mod observed;

use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use observed::Observed;
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, Response, ResponseRegisters,
};
use pdma_model_device::Device;
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
        .register_offset()
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

fn setup() -> TestResult<Device<Observed>> {
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x3000,
            MemoryAlignment::ONE,
        )?])?
        .try_install(&memory)?;
    let mut device = Device::new(
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

fn reached(device: &mut Device<Observed>, boundary: Boundary) -> TestResult<bool> {
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
                    operation.step(&mut hardware, &mut device),
                    StepOutcome::Progressed
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
            // When: reset is immediately followed by reconfiguration and a fresh command.
            let memory_base = replace(&mut device, reset_index)?;
            for _ in 0..512 {
                assert!(matches!(
                    operation.step(&mut hardware, &mut device),
                    StepOutcome::Progressed | StepOutcome::Pending(())
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
            for index in 0..3 {
                assert_eq!(
                    word(
                        &mut device,
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
                    .read_typed::<[u8; Response::RECORD_SIZE]>(
                        MemoryAddress::new(0x1100 + u64::try_from(index)? * 0x1000),
                        MemoryAlignment::ONE,
                    )?;
                let response = Response::from_bytes(&raw)?;
                assert_eq!(
                    response.id,
                    if index == reset_index {
                        99
                    } else {
                        u64::try_from(index)?
                    }
                );
                assert_eq!(response.bytes_completed, 512);
            }
        }
    }
    Ok(())
}

fn replace(device: &mut Device<Observed>, reset_index: usize) -> TestResult<u64> {
    device.write(
        register(
            reset_index,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::RESET.offset(),
        )?,
        &1_u32.to_le_bytes(),
    )?;
    configure(device, reset_index)?;
    let memory_base = 0x1000 + u64::try_from(reset_index)? * 0x1000;
    let fresh = pdma_model_contract::Command {
        id: 99,
        flags: pdma_model_contract::CompletionFlagsValues { ioc: true },
        source: pdma_model_contract::Endpoint::linear(memory_base + 0x200, 512),
        destination: pdma_model_contract::Endpoint::linear(memory_base + 0x800, 512),
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
