use super::*;

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
            CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 2,
                index: 0,
                offset: 0,
            })
        } else {
            CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1200,
                offset: 0,
            })
        };
        let destination = if read_fault {
            CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            })
        } else {
            CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x1400,
                count: 2,
                index: 0,
                offset: 0,
            })
        };
        submit(
            &mut device,
            &memory,
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: 3,
                copy_length: 8,
                sg_context: 0,
                source,
                destination,
                ..pdma_model_contract::dma::CopyCommand::ZERO
            }),
        )?;
        // When: the second payload access fails.
        run(&mut device)?;
        // Then: completed bytes report only the successful write and the untouched suffix remains.
        let status = if read_fault {
            EventCode::ReadError
        } else {
            EventCode::WriteError
        };
        assert_eq!(
            event(&memory)?,
            Event {
                id: 3,
                code: status,
                info: if read_fault {
                    EventInfoValues::ReadError(ErrorInfo { bytes_completed: 3 })
                } else {
                    EventInfoValues::WriteError(ErrorInfo { bytes_completed: 3 })
                },
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
fn zero_length_does_not_access_lists_or_payload() -> TestResult {
    // Given: a zero-length request with inaccessible SG tables.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(
        &mut device,
        &memory,
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 4,
            copy_length: 0,
            sg_context: 0,
            source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                list_address: 0x8000,
                count: 2,
                index: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
                list_address: u64::MAX,
                count: 0,
                index: 0,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    // When: hardware accepts the empty copy range.
    run(&mut device)?;
    // Then: the empty command retires without payload access or an event.
    support::assert_no_event(&mut device)?;
    assert!(!device.irq());
    Ok(())
}
