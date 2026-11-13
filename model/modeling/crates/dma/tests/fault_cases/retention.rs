//! Per-channel retained data is a bounded resource independent of shared execution.
use super::*;
use pdma_model_contract::dma::{
    ChannelAdmin, ChannelConfiguration, ChannelId, CopyCommand, LinearEndpoint, WordTarget,
    WordTargetWidth, WordWrite,
};

#[derive(Clone, Copy)]
struct Buffers {
    ring: u64,
    event: u64,
    source: u64,
    destination: u64,
    fence: u64,
}

impl Buffers {
    fn at(index: usize) -> TestResult<Self> {
        let base = 0x1000 + u64::try_from(index)? * 0x1000;
        Ok(Self {
            ring: base,
            event: base + 0x100,
            source: base + 0x200,
            destination: base + 0x400,
            fence: base + 0x600,
        })
    }
}

fn register_base(index: usize, count: usize) -> TestResult<usize> {
    ChannelId::new(index)
        .register_offset(count)
        .ok_or_else(|| "channel layout".into())
}

fn configure_channel(device: &mut Dma<Paged>, index: usize, count: usize) -> TestResult {
    let buffers = Buffers::at(index)?;
    let base = register_base(index, count)?;
    let command = base + ChannelBlock::COMMAND.offset();
    let event = base + ChannelBlock::EVENT.offset();
    for (offset, value) in [
        (command + CommandRegisters::BASE.offset(), buffers.ring),
        (event + EventRegisters::BASE.offset(), buffers.event),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (command + CommandRegisters::CAPACITY.offset(), 2_u32),
        (event + EventRegisters::CAPACITY.offset(), 1),
        (
            base + ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    Ok(())
}

fn publish(device: &mut Dma<Paged>, index: usize, count: usize, id: u64) -> TestResult {
    let buffers = Buffers::at(index)?;
    let Command::Copy(template) = Command::ZERO else {
        return Err("copy template".into());
    };
    let copy = Command::Copy(CopyCommand {
        id,
        copy_length: u64::try_from(pdma_model_dma::COPY_CHUNK_BYTES)?,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: buffers.source,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: buffers.destination,
            offset: 0,
        }),
        ..template
    });
    let fence = Command::Fence(FenceCommand {
        header: CommandHeader {
            opcode: Opcode::Fence,
        },
        id,
        word: WordWrite {
            target: WordTarget {
                address: buffers.fence,
                context: 0,
                width: WordTargetWidth::Bits64,
            },
            value: id,
        },
    });
    for (slot, command) in [copy, fence].iter().enumerate() {
        device.memory.memory.write(
            (buffers.ring + u64::try_from(slot * Command::RECORD_SIZE)?).into(),
            MemoryAlignment::ONE,
            &command.to_bytes()?,
        )?;
    }
    device.write(
        register_base(index, count)?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    Ok(())
}

fn marker(device: &Dma<Paged>, index: usize) -> TestResult<u64> {
    Ok(u64::from_le_bytes(
        device
            .memory
            .memory
            .read_buffer::<[u8; 8]>(Buffers::at(index)?.fence.into(), MemoryAlignment::ONE)?,
    ))
}

fn recovery(index: usize, count: usize, offset: usize) -> TestResult<usize> {
    Ok(register_base(index, count)?
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::RECOVERY.offset()
        + offset)
}

fn destination(
    device: &Dma<Paged>,
    index: usize,
) -> TestResult<[u8; pdma_model_dma::COPY_CHUNK_BYTES]> {
    let mut bytes = [0; pdma_model_dma::COPY_CHUNK_BYTES];
    device.memory.memory.read(
        Buffers::at(index)?.destination.into(),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    Ok(bytes)
}

fn with_channels(count: usize) -> TestResult<Fixture> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000 * u64::try_from(count)?,
        MemoryAlignment::ONE,
    )?])?;
    let dma = Dma::new(
        Paged {
            memory,
            missing: std::cell::Cell::new(None),
            faults: std::cell::Cell::new(0),
            accesses: std::cell::Cell::new(0),
            reads: std::cell::RefCell::new(Vec::new()),
        },
        count,
    )?;
    let pipeline = dma.hardware();
    Ok(Fixture {
        dma,
        pipeline,
        operation: hw_model_pipeline::HardwareDriver::new(),
    })
}

#[test]
fn retained_writes_for_all_channel_counts_preserve_peer_progress_and_reset_isolation() -> TestResult
{
    for count in 1..=8 {
        let mut fixture = with_channels(count)?;
        // Given: every channel owns an independent retained-write opportunity.
        for index in 0..count {
            configure_channel(&mut fixture, index, count)?;
            fixture.memory.memory.write(
                Buffers::at(index)?.source.into(),
                MemoryAlignment::ONE,
                &vec![u8::try_from(index + 1)?; pdma_model_dma::COPY_CHUNK_BYTES],
            )?;
        }
        // Park all but the final channel, with different successfully read data.
        for index in 0..count - 1 {
            fixture
                .memory
                .missing
                .set(Some((Buffers::at(index)?.destination, true)));
            publish(&mut fixture, index, count, u64::try_from(index + 1)?)?;
            let fault = recovery(index, count, ChannelRecovery::FAULT_STATUS.offset())?;
            fixture.until(|dma| Ok(read_word(dma, fault)? == 1))?;
        }
        let peer = count - 1;
        publish(&mut fixture, peer, count, u64::try_from(count)?)?;
        fixture.until(|dma| Ok(marker(dma, peer)? == u64::try_from(count)?))?;
        // Then: a healthy peer completes while every other owner remains parked.
        for index in 0..peer {
            assert_eq!(
                read_word(
                    &mut fixture,
                    recovery(index, count, ChannelRecovery::FAULT_STATUS.offset())?
                )?,
                1
            );
            assert_eq!(marker(&fixture, index)?, 0);
            fixture.memory.memory.write(
                Buffers::at(index)?.source.into(),
                MemoryAlignment::ONE,
                &vec![0xee; pdma_model_dma::COPY_CHUNK_BYTES],
            )?;
        }
        fixture.memory.missing.set(None);
        if count > 1 {
            // Reset one parked owner while the peer's Fence can still be resident.
            let reset = ChannelId::new(0).admin_offset(count).ok_or("admin")?
                + ChannelAdmin::RESET.offset();
            fixture.write(reset, &1_u32.to_le_bytes())?;
            fixture.until(|dma| Ok(read_word(dma, reset)? == 0))?;
            configure_channel(&mut fixture, 0, count)?;
            publish(&mut fixture, 0, count, 100)?;
            fixture.until(|dma| Ok(marker(dma, 0)? == 100))?;
            assert_eq!(
                destination(&fixture, 0)?,
                [0xee; pdma_model_dma::COPY_CHUNK_BYTES]
            );
        }
        // Each surviving owner resumes its original read bytes, not the changed source.
        for index in 1..peer {
            fixture.write(
                recovery(index, count, ChannelRecovery::RESUME.offset())?,
                &1_u32.to_le_bytes(),
            )?;
            fixture.until(|dma| Ok(marker(dma, index)? == u64::try_from(index + 1)?))?;
        }
        for index in 0..count {
            let expected = if count > 1 && index == 0 {
                0xee
            } else {
                u8::try_from(index + 1)?
            };
            assert_eq!(
                destination(&fixture, index)?,
                [expected; pdma_model_dma::COPY_CHUNK_BYTES]
            );
            assert_eq!(
                read_word(
                    &mut fixture,
                    register_base(index, count)?
                        + ChannelBlock::EVENT.offset()
                        + EventRegisters::PRODUCER.offset()
                )?,
                0
            );
        }
    }
    Ok(())
}
