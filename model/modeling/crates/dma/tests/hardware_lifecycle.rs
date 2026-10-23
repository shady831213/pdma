//! Hardware ownership boundaries and admission exclusion.
/// Shared raw-bus fixtures.
pub mod support;

mod hardware_lifecycle {
    mod stages;

    use crate::support::{TestResult, command, read_word, setup, submit};
    use hardware_interface_pipeline::{HardwareDriver, Operation, StepOutcome, Then};
    use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
    use pdma_model_contract::dma::{
        ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, ContextId, DmaAdmin,
        DmaDiscovery, Endpoint, Event, EventCode, EventRegisters,
    };
    use pdma_model_dma::{CommandRequest, Fetch};

    use self::stages::{PendingTail, Stalled};

    const RESET: usize = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    const CONSUMER: usize = ChannelBlock::ALIGNMENT
        + ChannelBlock::COMMAND.offset()
        + CommandRegisters::CONSUMER.offset();
    const PRODUCER: usize =
        ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset();

    #[test]
    fn irq_causes_refresh_after_publication_while_data_tail_stalls() -> TestResult {
        // Given: a publishing execution graph with an indefinitely stalled final stage.
        let (mut dma, memory) = setup(0x1100, 2)?;
        let mut descriptor = command();
        crate::support::copy_mut(&mut descriptor)?.source = Endpoint::linear(0x9000, 0);
        submit(&mut dma, &memory, descriptor)?;
        let mut hardware =
            dma.hardware_with(Then::new(crate::support::chunk_pipeline(), PendingTail));
        let mut driver = HardwareDriver::new();
        let causes = ChannelBlock::ALIGNMENT
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::IRQ.offset()
            + pdma_model_contract::dma::ChannelInterrupts::CAUSE.offset();
        // When: the data graph publishes while its finite invocation remains unfinished.
        for _ in 0..128 {
            assert!(matches!(
                driver.step(&mut hardware, &mut dma)?,
                StepOutcome::InProgress
            ));
            if read_word(&mut dma, PRODUCER)? == 1 {
                break;
            }
            assert!([0, 4].contains(&read_word(&mut dma, causes)?));
        }
        // Then: publication exposes the event cause even though the tail cannot finish.
        assert_eq!(read_word(&mut dma, PRODUCER)?, 1);
        assert_eq!(read_word(&mut dma, causes)?, 5);
        assert!(dma.irq());
        assert!(!driver.is_complete());
        Ok(())
    }

    #[test]
    fn reset_retires_an_accepted_invocation_when_its_stalled_stage_observes_reset() -> TestResult {
        // Given: accepted work whose execution graph stalls until reset.
        let (mut dma, memory) = setup(0x1100, 2)?;
        submit(&mut dma, &memory, command())?;
        let cleanup = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut hardware = dma.hardware_with(Stalled(None, std::rc::Rc::clone(&cleanup)));
        let mut driver = HardwareDriver::new();
        for _ in 0..64 {
            assert!(matches!(
                driver.step(&mut hardware, &mut dma)?,
                StepOutcome::InProgress
            ));
            if cleanup.get() == 1 {
                break;
            }
        }
        assert_eq!(read_word(&mut dma, CONSUMER)?, 1);
        assert_eq!(cleanup.get(), 1);
        // When: reset is requested while accepted ownership remains.
        dma.write(RESET, &1_u32.to_le_bytes())?;
        assert_eq!(read_word(&mut dma, RESET)?, 1);
        assert!(matches!(
            driver.step(&mut hardware, &mut dma)?,
            StepOutcome::InProgress
        ));
        assert_eq!(cleanup.get(), 2);
        assert_eq!(read_word(&mut dma, RESET)?, 1);
        for _ in 0..64 {
            assert!(matches!(
                driver.step(&mut hardware, &mut dma)?,
                StepOutcome::InProgress
            ));
            if read_word(&mut dma, RESET)? == 0 {
                break;
            }
        }
        // Then: the stage's own terminal step acknowledges reset without completing data.
        assert_eq!(read_word(&mut dma, RESET)?, 0);
        assert_eq!(cleanup.get(), 3);
        assert!(!driver.is_complete());
        assert_eq!(read_word(&mut dma, PRODUCER)?, 0);
        Ok(())
    }

    #[test]
    fn stale_fetch_leaves_the_next_abort_target_for_fresh_arbitration() -> TestResult {
        // Given: two Abort targets and a snapshot of the original head.
        let (mut dma, memory) = setup(0x1100, 2)?;
        let target = {
            let original = command();
            let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) = original.payload
            else {
                return Err("copy fixture required".into());
            };
            Command::copy(
                original.id,
                pdma_model_contract::dma::CopyCommand {
                    source: Endpoint {
                        context: 7,
                        ..Endpoint::linear(0x1200, 0)
                    },
                    ..copy
                },
            )
        };
        submit(&mut dma, &memory, target)?;
        memory.write(
            (0x1000 + u64::try_from(Command::RECORD_SIZE)?).into(),
            MemoryAlignment::ONE,
            &target.to_bytes()?,
        )?;
        dma.write(
            ChannelBlock::ALIGNMENT
                + ChannelBlock::COMMAND.offset()
                + CommandRegisters::PRODUCER.offset(),
            &2_u32.to_le_bytes(),
        )?;
        let id = ChannelId::new(0);
        let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(id)? else {
            return Err("missing snapshot".into());
        };
        dma.abort_context(ContextId::new(7), &[id])?;
        let mut hardware = dma.hardware();
        let mut driver = HardwareDriver::new();
        // When: normal admission discards one Abort target before the stale snapshot is used.
        for _ in 0..64 {
            let _ = driver.step(&mut hardware, &mut dma)?;
            if read_word(&mut dma, CONSUMER)? == 1 {
                break;
            }
        }
        assert_eq!(read_word(&mut dma, CONSUMER)?, 1);
        let result =
            Operation::new(Some(CommandRequest::Fetch(snapshot))).step(&mut Fetch, &mut dma)?;
        // Then: stale Fetch leaves the next target untouched until normal admission selects it.
        assert!(matches!(result, StepOutcome::Complete(Ok(ref flow)) if flow.data().is_none()));
        assert_eq!(read_word(&mut dma, CONSUMER)?, 1);
        for _ in 0..128 {
            let _ = driver.step(&mut hardware, &mut dma)?;
            if dma.context_status(ContextId::new(7))? {
                break;
            }
        }
        assert_eq!(read_word(&mut dma, CONSUMER)?, 2);
        assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
        assert_eq!(read_word(&mut dma, PRODUCER)?, 0);
        Ok(())
    }

    #[test]
    fn preaccept_reset_refreshes_identity_through_normal_and_error_completion() -> TestResult {
        for failure in [false, true] {
            for prefix in 0..16 {
                // Given: every boundary before a candidate becomes accepted hardware work.
                let (mut dma, memory) = setup(0x1100, 2)?;
                submit(&mut dma, &memory, command())?;
                let mut path = dma.hardware();
                let mut invocation = HardwareDriver::new();
                for _ in 0..prefix {
                    assert!(matches!(
                        invocation.step(&mut path, &mut dma)?,
                        StepOutcome::InProgress
                    ));
                    if read_word(&mut dma, CONSUMER)? == 1 {
                        break;
                    }
                }
                if read_word(&mut dma, CONSUMER)? == 1 {
                    break;
                }
                // When: reset completes and software configures a new generation before acceptance.
                dma.write(RESET, &1_u32.to_le_bytes())?;
                for _ in 0..64 {
                    if read_word(&mut dma, RESET)? == 0 {
                        break;
                    }
                    let _ = invocation.step(&mut path, &mut dma)?;
                }
                assert_eq!(read_word(&mut dma, RESET)?, 0);
                let base = ChannelBlock::ALIGNMENT;
                for (offset, value) in [
                    (
                        base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                        0x1000_u64,
                    ),
                    (
                        base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                        0x1100,
                    ),
                ] {
                    dma.write(offset, &value.to_le_bytes())?;
                }
                for (offset, value) in [
                    (
                        base + ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                        2_u32,
                    ),
                    (
                        base + ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
                        2,
                    ),
                    (
                        base + ChannelBlock::CONTROL.offset()
                            + ChannelRegisters::CONFIG.offset()
                            + pdma_model_contract::dma::ChannelConfiguration::ENABLE.offset(),
                        1,
                    ),
                ] {
                    dma.write(offset, &value.to_le_bytes())?;
                }
                let fresh = {
                    let original = command();
                    let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) =
                        original.payload
                    else {
                        return Err("copy fixture required".into());
                    };
                    Command::copy(
                        99,
                        pdma_model_contract::dma::CopyCommand {
                            source: Endpoint::linear(if failure { 0x9000 } else { 0x1200 }, 0),
                            ..copy
                        },
                    )
                };
                memory.write(0x1200.into(), MemoryAlignment::ONE, &[9; 4])?;
                submit(&mut dma, &memory, fresh)?;
                for _ in 0..128 {
                    assert!(matches!(
                        invocation.step(&mut path, &mut dma)?,
                        StepOutcome::InProgress
                    ));
                }
                assert!(!invocation.is_complete());
                assert_eq!(
                    read_word(&mut dma, PRODUCER)?,
                    u32::from(failure),
                    "prefix {prefix}, failure {failure}"
                );
                if failure {
                    let mut bytes = [0; Event::RECORD_SIZE];
                    memory.read(0x1100.into(), MemoryAlignment::ONE, &mut bytes)?;
                    let event = Event::from_bytes(&bytes)?;
                    assert_eq!((event.id, event.code), (fresh.id, EventCode::ReadError));
                } else {
                    assert_eq!(
                        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
                        [9; 4]
                    );
                }
            }
        }
        Ok(())
    }
}
