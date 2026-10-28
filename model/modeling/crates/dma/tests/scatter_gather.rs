//! Independent endpoint layouts share the same hardware copy pipeline.

use pdma_model_contract::dma::{
    Command, CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint,
    ScatterGatherEndpoint, Segment,
};

/// Shared hardware test fixtures.
pub mod support;

use hw_model_io::memory::SharedMemory;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use support::{TestResult, run, setup, submit};

fn write_segments(memory: &SharedMemory, base: u64, segments: &[Segment]) -> TestResult {
    for (index, segment) in segments.iter().enumerate() {
        memory.write(
            MemoryAddress::new(base + u64::try_from(index * Segment::RECORD_SIZE)?),
            MemoryAlignment::ONE,
            &segment.to_bytes()?,
        )?;
    }
    Ok(())
}

#[test]
fn independent_endpoint_modes_copy_across_different_boundaries() -> TestResult {
    for (gather, scatter) in [(false, false), (true, false), (false, true), (true, true)] {
        // Given: source boundaries 3+0+5 and destination boundaries 2+6.
        let (mut device, memory) = setup(0x1100, 2)?;
        memory.write(
            MemoryAddress::new(0x1200),
            MemoryAlignment::ONE,
            b"abcdefgh",
        )?;
        memory.write(MemoryAddress::new(0x1240), MemoryAlignment::ONE, b"defgh")?;
        write_segments(
            &memory,
            0x1400,
            &[
                Segment {
                    address: 0x1200,
                    length: 3,
                },
                Segment {
                    address: u64::MAX,
                    length: 0,
                },
                Segment {
                    address: 0x1240,
                    length: 5,
                },
            ],
        )?;
        write_segments(
            &memory,
            0x1500,
            &[
                Segment {
                    address: 0x1300,
                    length: 2,
                },
                Segment {
                    address: 0x1340,
                    length: 6,
                },
            ],
        )?;
        let source = if gather {
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 3,
                index: 0,
                offset: 0,
            })
        } else {
            CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1200,
                offset: 0,
            })
        };
        let destination = if scatter {
            CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1500,
                count: 2,
                index: 0,
                offset: 0,
            })
        } else {
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            })
        };
        submit(
            &mut device,
            &memory,
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: 7,
                copy_length: 8,
                sg_context: 0,
                source,
                destination,
                ..pdma_model_contract::dma::CopyCommand::ZERO
            }),
        )?;
        // When: the normal composed hardware executes one command.
        run(&mut device)?;
        // Then: all modes produce the same logical byte stream without an event.
        let mut bytes = [0; 8];
        if scatter {
            memory.read(
                MemoryAddress::new(0x1300),
                MemoryAlignment::ONE,
                bytes.get_mut(..2).ok_or("prefix")?,
            )?;
            memory.read(
                MemoryAddress::new(0x1340),
                MemoryAlignment::ONE,
                bytes.get_mut(2..).ok_or("suffix")?,
            )?;
        } else {
            memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
        }
        assert_eq!(&bytes, b"abcdefgh");
        support::assert_no_event(&mut device)?;
        assert!(!device.irq());
    }
    Ok(())
}

#[test]
fn scratch_buffer_size_does_not_limit_command_length() -> TestResult {
    // Given: a 769-byte linear command, larger than one hardware scratch block.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1400),
        MemoryAlignment::ONE,
        &[0xa5; 769],
    )?;
    submit(
        &mut device,
        &memory,
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 8,
            copy_length: 769,
            sg_context: 0,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1400,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1800,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    // When: hardware executes through multiple copy iterations.
    run(&mut device)?;
    // Then: every byte is copied without an event.
    let mut bytes = [0; 769];
    memory.read(MemoryAddress::new(0x1800), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0xa5; 769]);
    support::assert_no_event(&mut device)?;
    Ok(())
}

#[test]
fn overlapping_segments_follow_execution_order() -> TestResult {
    // Given: the first write overwrites the next source segment.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, b"abcdef")?;
    write_segments(
        &memory,
        0x1400,
        &[
            Segment {
                address: 0x1200,
                length: 2,
            },
            Segment {
                address: 0x1202,
                length: 2,
            },
        ],
    )?;
    submit(
        &mut device,
        &memory,
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 5,
            copy_length: 4,
            sg_context: 0,
            source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 2,
                index: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1202,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    // When: hardware processes the stream without detecting or rejecting overlap.
    run(&mut device)?;
    // Then: the later read observes the earlier write, not a whole-stream snapshot.
    assert_eq!(
        &memory.read_buffer::<[u8; 6]>(MemoryAddress::new(0x1200), MemoryAlignment::ONE)?,
        b"ababab"
    );
    support::assert_no_event(&mut device)?;
    Ok(())
}
