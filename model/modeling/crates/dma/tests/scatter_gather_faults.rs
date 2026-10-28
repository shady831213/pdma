//! Validation and partial-completion errors without hidden rollback or overlap policy.

use pdma_model_contract::dma::{
    Command, CopyCommandDestinationValues, CopyCommandSourceValues, ErrorInfo, Event, EventCode,
    EventInfoValues, EventRaw, LinearEndpoint, ScatterGatherEndpoint, Segment,
};

/// Shared hardware test fixtures.
pub mod support;

use hw_model_io::memory::SharedMemory;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use support::{TestResult, run, setup, submit};

fn segments(memory: &SharedMemory, entries: &[Segment]) -> TestResult {
    for (index, entry) in entries.iter().enumerate() {
        memory.write(
            MemoryAddress::new(0x1400 + u64::try_from(index * Segment::RECORD_SIZE)?),
            MemoryAlignment::ONE,
            &entry.to_bytes()?,
        )?;
    }
    Ok(())
}

fn event(memory: &SharedMemory) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::from_bytes(
        memory
            .read_buffer::<EventRaw>(MemoryAddress::new(0x1100), MemoryAlignment::ONE)?
            .as_ref(),
    )?)
}

#[test]
fn insufficient_selected_range_leaves_payload_untouched() -> TestResult {
    // Given: an unreadable four-byte gathered source and a five-byte destination.
    let (mut device, memory) = setup(0x1100, 2)?;
    segments(
        &memory,
        &[Segment {
            address: 0x8000,
            length: 4,
        }],
    )?;
    memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, b"guard")?;
    submit(
        &mut device,
        &memory,
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 1,
            copy_length: 5,
            sg_context: 0,
            source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 1,
                index: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    // When: hardware validates the endpoint totals.
    run(&mut device)?;
    // Then: mismatch wins before any payload access and no destination byte changes.
    assert_eq!(
        event(&memory)?,
        Event {
            id: 1,
            code: EventCode::LengthMismatch,
            info: EventInfoValues::LengthMismatch(ErrorInfo { bytes_completed: 0 }),
        }
    );
    assert_eq!(
        &memory.read_buffer::<[u8; 5]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        b"guard"
    );
    Ok(())
}

#[test]
fn list_errors_and_length_overflow_precede_payload_writes() -> TestResult {
    for (source, destination, expected) in failure_cases() {
        // Given: an unreadable/misaligned list, overflowing total, or overflowing list extent.
        let (mut device, memory) = setup(0x1100, 2)?;
        segments(
            &memory,
            &[
                Segment {
                    address: u64::MAX - 1,
                    length: 4,
                },
                Segment {
                    address: 0x1200,
                    length: 1,
                },
            ],
        )?;
        memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, b"keep")?;
        submit(
            &mut device,
            &memory,
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: 2,
                copy_length: 4,
                sg_context: 0,
                source,
                destination,
                ..pdma_model_contract::dma::CopyCommand::ZERO
            }),
        )?;
        // When: the real hardware pipeline runs its preflight.
        run(&mut device)?;
        // Then: a typed command failure is published without modifying payload.
        assert_eq!(
            event(&memory)?,
            Event {
                id: 2,
                code: expected,
                info: match expected {
                    EventCode::InvalidLength =>
                        EventInfoValues::InvalidLength(ErrorInfo { bytes_completed: 0 }),
                    EventCode::ListReadError =>
                        EventInfoValues::ListReadError(ErrorInfo { bytes_completed: 0 }),
                    _ => return Err("unexpected test status".into()),
                },
            }
        );
        assert_eq!(
            &memory.read_buffer::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
            b"keep"
        );
    }
    Ok(())
}

mod scatter_gather_faults {
    use super::*;
    mod payload;
}

const fn failure_cases() -> [(
    CopyCommandSourceValues,
    CopyCommandDestinationValues,
    EventCode,
); 5] {
    [
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x8000,
                count: 1,
                index: 0,
                offset: 0,
            }),
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            EventCode::ListReadError,
        ),
        (
            CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1200,
                offset: 0,
            }),
            CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x8000,
                count: 1,
                index: 0,
                offset: 0,
            }),
            EventCode::ListReadError,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1401,
                count: 1,
                index: 0,
                offset: 0,
            }),
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            EventCode::ListReadError,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 2,
                index: 0,
                offset: 0,
            }),
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            EventCode::InvalidLength,
        ),
        (
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0,
                count: u64::MAX,
                index: u64::MAX - 1,
                offset: 0,
            }),
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            EventCode::InvalidLength,
        ),
    ]
}
