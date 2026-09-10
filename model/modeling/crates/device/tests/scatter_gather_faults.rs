//! Validation and partial-completion errors without hidden rollback or overlap policy.

/// Shared hardware test fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{Command, Endpoint, Response, ResponseStatus, Segment};
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

fn response(memory: &SharedMemory) -> Result<Response, Box<dyn std::error::Error>> {
    Ok(Response::from_bytes(
        &memory.read_typed::<[u8; Response::RECORD_SIZE]>(
            MemoryAddress::new(0x1100),
            MemoryAlignment::ONE,
        )?,
    )?)
}

#[test]
fn mismatched_totals_leave_payload_untouched() -> TestResult {
    // Given: an unreadable four-byte gathered source and a five-byte destination.
    let (mut device, memory) = setup(0x1100, 2)?;
    segments(
        &memory,
        &[Segment {
            address: u64::MAX,
            length: 4,
        }],
    )?;
    memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, b"guard")?;
    submit(
        &mut device,
        &memory,
        Command {
            id: 1,
            source: Endpoint::scatter_gather(0x1400, 1),
            destination: Endpoint::linear(0x1300, 5),
        },
    )?;
    // When: hardware validates the endpoint totals.
    run(&mut device)?;
    // Then: mismatch wins before any payload access and no destination byte changes.
    assert_eq!(
        response(&memory)?,
        Response {
            id: 1,
            status: ResponseStatus::LengthMismatch,
            bytes_completed: 0
        }
    );
    assert_eq!(
        &memory.read_typed::<[u8; 5]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        b"guard"
    );
    Ok(())
}

#[test]
fn list_errors_and_length_overflow_precede_payload_writes() -> TestResult {
    for (source, destination, expected) in [
        (
            Endpoint::scatter_gather(0x8000, 1),
            Endpoint::linear(0x1300, 4),
            ResponseStatus::ListReadFault,
        ),
        (
            Endpoint::linear(0x1200, 4),
            Endpoint::scatter_gather(0x8000, 1),
            ResponseStatus::ListReadFault,
        ),
        (
            Endpoint::scatter_gather(0x1401, 1),
            Endpoint::linear(0x1300, 4),
            ResponseStatus::ListReadFault,
        ),
        (
            Endpoint::scatter_gather(0x1400, 2),
            Endpoint::linear(0x1300, 4),
            ResponseStatus::InvalidLength,
        ),
        (
            Endpoint::scatter_gather(0, u64::MAX),
            Endpoint::linear(0x1300, 4),
            ResponseStatus::InvalidLength,
        ),
    ] {
        // Given: an unreadable/misaligned list, overflowing total, or overflowing list extent.
        let (mut device, memory) = setup(0x1100, 2)?;
        segments(
            &memory,
            &[
                Segment {
                    address: 0x1200,
                    length: u64::MAX,
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
            Command {
                id: 2,
                source,
                destination,
            },
        )?;
        // When: the real hardware pipeline runs its preflight.
        run(&mut device)?;
        // Then: a typed command failure is published without modifying payload.
        assert_eq!(
            response(&memory)?,
            Response {
                id: 2,
                status: expected,
                bytes_completed: 0
            }
        );
        assert_eq!(
            &memory.read_typed::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
            b"keep"
        );
    }
    Ok(())
}

#[test]
fn payload_fault_reports_written_prefix_without_rollback() -> TestResult {
    for read_fault in [true, false] {
        // Given: the second segment faults after a three-byte prefix succeeds.
        let (mut device, memory) = setup(0x1100, 2)?;
        memory.write(
            MemoryAddress::new(0x1200),
            MemoryAlignment::ONE,
            b"abcdefgh",
        )?;
        memory.write(
            MemoryAddress::new(0x1300),
            MemoryAlignment::ONE,
            b"--------",
        )?;
        let first = if read_fault { 0x1200 } else { 0x1300 };
        segments(
            &memory,
            &[
                Segment {
                    address: first,
                    length: 3,
                },
                Segment {
                    address: 0x8000,
                    length: 5,
                },
            ],
        )?;
        let source = if read_fault {
            Endpoint::scatter_gather(0x1400, 2)
        } else {
            Endpoint::linear(0x1200, 8)
        };
        let destination = if read_fault {
            Endpoint::linear(0x1300, 8)
        } else {
            Endpoint::scatter_gather(0x1400, 2)
        };
        submit(
            &mut device,
            &memory,
            Command {
                id: 3,
                source,
                destination,
            },
        )?;
        // When: the second payload access fails.
        run(&mut device)?;
        // Then: completed bytes report only the successful write and the untouched suffix remains.
        let status = if read_fault {
            ResponseStatus::ReadFault
        } else {
            ResponseStatus::WriteFault
        };
        assert_eq!(
            response(&memory)?,
            Response {
                id: 3,
                status,
                bytes_completed: 3
            }
        );
        assert_eq!(
            &memory.read_typed::<[u8; 8]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
            b"abc-----"
        );
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn zero_total_does_not_access_payload_or_empty_list_addresses() -> TestResult {
    // Given: zero-length spans with deliberately unusable addresses.
    let (mut device, memory) = setup(0x1100, 2)?;
    segments(
        &memory,
        &[
            Segment {
                address: u64::MAX,
                length: 0,
            },
            Segment {
                address: 0x8000,
                length: 0,
            },
        ],
    )?;
    submit(
        &mut device,
        &memory,
        Command {
            id: 4,
            source: Endpoint::scatter_gather(0x1400, 2),
            destination: Endpoint::scatter_gather(u64::MAX, 0),
        },
    )?;
    // When: the totals match at zero.
    run(&mut device)?;
    // Then: one successful response reports zero completed bytes.
    assert_eq!(
        response(&memory)?,
        Response {
            id: 4,
            status: ResponseStatus::Success,
            bytes_completed: 0
        }
    );
    assert!(device.irq());
    Ok(())
}
