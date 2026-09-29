//! Runtime memory isolation through the composed DMA execution stages.
mod lifecycle;
/// Shared hardware fixtures for Function memory tests.
mod support;

use hardware_interface_pipeline::Operation;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, CommandRegisters, Endpoint, ResponseRegisters, ResponseStatus, Segment,
};
use support::{Fixture, TestResult, advance, bytes, command, publish, response, setup};

#[derive(Clone, Copy, Debug)]
enum Path {
    Command,
    List,
    Read,
    Write,
    Response,
}

#[test]
fn command_lists_payload_and_response_use_the_same_channel_owner() -> TestResult {
    // Given: identical addresses contain different commands, lists and payloads per Function.
    let Fixture {
        mut dma, spaces, ..
    } = setup()?;
    for (index, memory) in spaces.iter().enumerate() {
        let source = 0x1200 + u64::try_from(index)? * 0x40;
        let destination = 0x1300 + u64::try_from(index)? * 0x40;
        memory.write_typed(
            MemoryAddress::new(source),
            MemoryAlignment::ONE,
            &[u8::try_from(index)? + 0x31; 8],
        )?;
        for (base, address) in [(0x1400, source), (0x1500, destination)] {
            memory.write_typed(
                MemoryAddress::new(base),
                MemoryAlignment::ONE,
                &Segment { address, length: 8 }.to_bytes()?,
            )?;
        }
        let mut work = command(100 + u64::try_from(index)?);
        work.source = Endpoint::scatter_gather(0x1400, 1);
        work.destination = Endpoint::scatter_gather(0x1500, 1);
        publish(&mut dma, memory, (ChannelId::new(index), work))?;
    }
    // When: the shared execution pipeline services both channels.
    let mut pipeline = dma.command_loop();
    advance(&mut dma, &mut pipeline, &mut Operation::new(()))?;
    // Then: each Function receives only its own data and completion at its list-selected address.
    for (index, memory) in spaces.iter().enumerate() {
        let destination = 0x1300 + u64::try_from(index)? * 0x40;
        assert_eq!(
            bytes::<8>(memory, destination)?,
            [u8::try_from(index)? + 0x31; 8]
        );
        assert_eq!(
            bytes::<8>(memory, 0x1340 - u64::try_from(index)? * 0x40)?,
            [0; 8]
        );
        let result = response(memory)?;
        assert_eq!(result.id, 100 + u64::try_from(index)?);
        assert_eq!(result.status, ResponseStatus::Success);
        assert_eq!(result.bytes_completed(), 8);
    }
    Ok(())
}

#[test]
fn another_functions_mapping_cannot_satisfy_a_dma_access() -> TestResult {
    for path in [
        Path::Command,
        Path::List,
        Path::Read,
        Path::Write,
        Path::Response,
    ] {
        // Given: only the peer maps the requested address; its bytes are a sentinel.
        let Fixture {
            mut dma,
            spaces: [local, peer],
            ..
        } = setup()?;
        peer.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x4000),
            256,
            MemoryAlignment::ONE,
        )?])?;
        peer.write_typed(
            MemoryAddress::new(0x4000),
            MemoryAlignment::ONE,
            &[0xa5; 256],
        )?;
        let mut work = command(91);
        let base = ChannelId::new(0).register_offset(2).ok_or("channel")?;
        match path {
            Path::Command => dma.write(
                base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                &0x4000_u64.to_le_bytes(),
            )?,
            Path::List => work.source = Endpoint::scatter_gather(0x4000, 1),
            Path::Read => work.source = Endpoint::linear(0x4000, 0),
            Path::Write => work.destination = Endpoint::linear(0x4000, 0),
            Path::Response => dma.write(
                base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
                &0x4000_u64.to_le_bytes(),
            )?,
        }
        publish(&mut dma, &local, (ChannelId::new(0), work))?;
        publish(&mut dma, &peer, (ChannelId::new(1), command(92)))?;
        // When: both channels execute and one encounters an unmapped local access.
        let mut pipeline = dma.command_loop();
        advance(&mut dma, &mut pipeline, &mut Operation::new(()))?;
        // Then: the foreign mapping stays untouched, and the peer still completes.
        assert_eq!(bytes::<256>(&peer, 0x4000)?, [0xa5; 256], "{path:?}");
        assert_eq!(response(&peer)?.id, 92);
        assert_eq!(response(&peer)?.status, ResponseStatus::Success);
        match path {
            Path::Command | Path::Response => {
                let mut fault = [0; 4];
                dma.read(
                    base + ChannelBlock::CONTROL.offset()
                        + pdma_model_contract::dma::ChannelRegisters::FATAL.offset(),
                    &mut fault,
                )?;
                assert_ne!(u32::from_le_bytes(fault), 0, "{path:?}");
            }
            Path::List => assert_eq!(response(&local)?.status, ResponseStatus::ListReadError),
            Path::Read => assert_eq!(response(&local)?.status, ResponseStatus::ReadError),
            Path::Write => assert_eq!(response(&local)?.status, ResponseStatus::WriteError),
        }
    }
    Ok(())
}
