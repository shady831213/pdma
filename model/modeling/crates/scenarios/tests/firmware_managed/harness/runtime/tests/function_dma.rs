//! Runtime memory isolation through the composed DMA execution stages.
mod lifecycle;
/// Shared hardware fixtures for Function memory tests.
mod support;

use hardware_interface_pipeline::HardwareDriver;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, CommandRegisters, Endpoint, EventCode, EventRegisters, Segment,
};
use support::{Fixture, TestResult, advance, bytes, command, publish, response, setup};

#[derive(Clone, Copy, Debug)]
enum Path {
    Command,
    List,
    Read,
    Write,
    Event,
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
        let pdma_model_contract::dma::CommandPayloadValues::Copy(ref mut copy) = work.payload
        else {
            return Err("expected Copy".into());
        };
        copy.source = Endpoint::scatter_gather(0x1400, 1);
        copy.destination = Endpoint::scatter_gather(0x1500, 1);
        publish(&mut dma, memory, (ChannelId::new(index), work))?;
    }
    // When: the shared execution pipeline services both channels.
    let mut pipeline = dma.hardware();
    advance(&mut dma, &mut pipeline, &mut HardwareDriver::new())?;
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
        assert_eq!(
            u64::from_le_bytes(bytes::<8>(memory, 0x1180)?),
            100 + u64::try_from(index)?
        );
        assert_eq!(bytes::<24>(memory, 0x1100)?, [0; 24]);
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
        Path::Event,
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
        let pdma_model_contract::dma::CommandPayloadValues::Copy(ref mut copy) = work.payload
        else {
            return Err("expected Copy".into());
        };
        let base = ChannelId::new(0).register_offset(2).ok_or("channel")?;
        match path {
            Path::Command => dma.write(
                base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                &0x4000_u64.to_le_bytes(),
            )?,
            Path::List => copy.source = Endpoint::scatter_gather(0x4000, 1),
            Path::Read => copy.source = Endpoint::linear(0x4000, 0),
            Path::Write => copy.destination = Endpoint::linear(0x4000, 0),
            Path::Event => {
                copy.source = Endpoint::linear(0x4000, 0);
                dma.write(
                    base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                    &0x4000_u64.to_le_bytes(),
                )?;
            }
        }
        publish(&mut dma, &local, (ChannelId::new(0), work))?;
        publish(&mut dma, &peer, (ChannelId::new(1), command(92)))?;
        // When: both channels execute and one encounters an unmapped local access.
        let mut pipeline = dma.hardware();
        advance(&mut dma, &mut pipeline, &mut HardwareDriver::new())?;
        // Then: the foreign mapping stays untouched, and the peer still completes.
        assert_eq!(bytes::<256>(&peer, 0x4000)?, [0xa5; 256], "{path:?}");
        assert_eq!(u64::from_le_bytes(bytes::<8>(&peer, 0x1180)?), 92);
        match path {
            Path::Command | Path::Event => {
                let mut fault = [0; 4];
                dma.read(
                    base + ChannelBlock::CONTROL.offset()
                        + pdma_model_contract::dma::ChannelRegisters::FATAL.offset(),
                    &mut fault,
                )?;
                assert_ne!(u32::from_le_bytes(fault), 0, "{path:?}");
            }
            Path::List => assert_eq!(response(&local)?.code, EventCode::ListReadError),
            Path::Read => assert_eq!(response(&local)?.code, EventCode::ReadError),
            Path::Write => assert_eq!(response(&local)?.code, EventCode::WriteError),
        }
    }
    Ok(())
}
