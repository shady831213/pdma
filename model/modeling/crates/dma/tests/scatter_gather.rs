//! Independent endpoint layouts share the same hardware copy pipeline.

/// Shared hardware test fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{Command, Endpoint, Response, ResponseStatus, Segment};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};
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

fn response(memory: &SharedMemory) -> Result<Response, Box<dyn std::error::Error>> {
    let bytes = memory.read_typed::<pdma_model_contract::dma::ResponseRaw>(
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
    )?;
    Ok(Response::from_bytes(bytes.as_ref())?)
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
            Endpoint::scatter_gather(0x1400, 3)
        } else {
            Endpoint::linear(0x1200, 0)
        };
        let destination = if scatter {
            Endpoint::scatter_gather(0x1500, 2)
        } else {
            Endpoint::linear(0x1300, 0)
        };
        submit(
            &mut device,
            &memory,
            Command {
                flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
                id: 7,
                ..Command::copy(
                    0,
                    pdma_model_contract::dma::CopyCommand {
                        copy_length: 8,
                        sg_context: 0,
                        source,
                        destination,
                    },
                )
            },
        )?;
        // When: the normal composed hardware executes one command.
        run(&mut device)?;
        // Then: all modes produce the same logical byte stream and one complete response.
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
        assert_eq!(
            response(&memory)?,
            Response {
                flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
                id: 7,
                status: ResponseStatus::Success,
                info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 8 }),
            }
        );
        assert!(device.irq());
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
        Command {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 8,
            ..Command::copy(
                0,
                pdma_model_contract::dma::CopyCommand {
                    copy_length: 769,
                    sg_context: 0,
                    source: Endpoint::linear(0x1400, 0),
                    destination: Endpoint::linear(0x1800, 0),
                },
            )
        },
    )?;
    // When: hardware executes through multiple copy iterations.
    run(&mut device)?;
    // Then: every byte is copied and the single response reports the complete length.
    let mut bytes = [0; 769];
    memory.read(MemoryAddress::new(0x1800), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0xa5; 769]);
    assert_eq!(
        response(&memory)?,
        Response {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 8,
            status: ResponseStatus::Success,
            info: ResponseInfoValues::Success(CompletionInfo {
                bytes_completed: 769
            }),
        }
    );
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
        Command {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 5,
            ..Command::copy(
                0,
                pdma_model_contract::dma::CopyCommand {
                    copy_length: 4,
                    sg_context: 0,
                    source: Endpoint::scatter_gather(0x1400, 2),
                    destination: Endpoint::linear(0x1202, 0),
                },
            )
        },
    )?;
    // When: hardware processes the stream without detecting or rejecting overlap.
    run(&mut device)?;
    // Then: the later read observes the earlier write, not a whole-stream snapshot.
    assert_eq!(
        &memory.read_typed::<[u8; 6]>(MemoryAddress::new(0x1200), MemoryAlignment::ONE)?,
        b"ababab"
    );
    assert_eq!(
        response(&memory)?,
        Response {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 5,
            status: ResponseStatus::Success,
            info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 4 }),
        }
    );
    Ok(())
}
