//! Context cleanup boundaries through the sustained hardware pipeline.

/// Shared DMA bus and memory fixtures.
pub mod support;

use hardware_interface_pipeline::StepOutcome;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{ChannelId, CommandRegisters, ContextId};
use support::{TestResult, command, setup, submit};

#[test]
fn abort_discards_unstarted_work_without_response_space() -> TestResult {
    // Given
    let (mut dma, memory) = setup(0x1100, 1)?;
    submit(&mut dma, &memory, command())?;
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..128 {
        let _ = operation.step(&mut pipeline, &mut dma)?;
    }
    let mut discarded = command();
    discarded.source.context = 7;
    discarded.destination.context = 7;
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &discarded.to_bytes()?,
    )?;
    let channel = ChannelId::new(0);
    let producer =
        channel.register_offset(1).ok_or("layout")? + CommandRegisters::PRODUCER.offset();
    dma.write(producer, &2_u32.to_le_bytes())?;
    // When
    dma.abort_context(ContextId::new(7), &[channel])?;
    for _ in 0..32 {
        let result = operation
            .step(&mut pipeline, &mut dma)
            .map_err(|e| format!("{e:?}"))?;
        assert!(!matches!(result, StepOutcome::Complete(_)));
    }
    // Then
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    let consumer =
        channel.register_offset(1).ok_or("layout")? + CommandRegisters::CONSUMER.offset();
    assert_eq!(support::read_word(&mut dma, consumer)?, 2);
    Ok(())
}

#[test]
fn abort_waits_for_accepted_command_and_preserves_its_normal_response() -> TestResult {
    // Given
    let (mut dma, memory) = setup(0x1100, 2)?;
    let mut input = command();
    input.source.context = 7;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[1, 2, 3, 4],
    )?;
    submit(&mut dma, &memory, input)?;
    let consumer =
        ChannelId::new(0).register_offset(1).ok_or("layout")? + CommandRegisters::CONSUMER.offset();
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..32 {
        let _ = operation.step(&mut pipeline, &mut dma)?;
        if support::read_word(&mut dma, consumer)? == 1 {
            break;
        }
    }
    assert_eq!(support::read_word(&mut dma, consumer)?, 1);
    // When
    dma.abort_context(ContextId::new(7), &[ChannelId::new(0)])?;
    // Then
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(false));
    for _ in 0..128 {
        let _ = operation.step(&mut pipeline, &mut dma)?;
    }
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [1, 2, 3, 4]);
    let mut response = [0; pdma_model_contract::dma::Response::RECORD_SIZE];
    memory.read(
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
        &mut response,
    )?;
    assert_eq!(
        pdma_model_contract::dma::Response::from_bytes(&response)?.id,
        input.id
    );
    Ok(())
}

#[test]
fn configured_ring_context_is_busy_even_while_channel_is_paused() -> TestResult {
    // Given
    let (mut dma, _) = setup(0x1100, 2)?;
    let enable = ChannelId::new(0).register_offset(1).ok_or("layout")?
        + pdma_model_contract::dma::ChannelBlock::CONTROL.offset();
    dma.write(enable, &0_u32.to_le_bytes())?;
    // When
    let result = dma.abort_context(ContextId::new(0), &[ChannelId::new(0)]);
    // Then
    assert_eq!(
        result,
        Err(pdma_model_dma::MaintenanceError::RingBound(ChannelId::new(
            0
        )))
    );
    Ok(())
}

#[test]
fn reset_interrupts_drain_but_waits_for_resident_cleanup() -> TestResult {
    use pdma_model_contract::dma::{DmaAdmin, DmaControl, DmaDiscovery};
    for whole_device in [false, true] {
        // Given: a fetched command is resident in a pending context Drain.
        let (mut dma, memory) = setup(0x1100, 2)?;
        let mut input = command();
        input.source.context = 7;
        submit(&mut dma, &memory, input)?;
        let id = ChannelId::new(0);
        let consumer = id.register_offset(1).ok_or("layout")? + CommandRegisters::CONSUMER.offset();
        let reset = if whole_device {
            DmaDiscovery::EXTENT + DmaAdmin::CONTROL.offset() + DmaControl::RESET.offset()
        } else {
            id.admin_offset(1).ok_or("layout")?
        };
        let mut pipeline = dma.hardware();
        let mut driver = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..128 {
            let _ = driver.step(&mut pipeline, &mut dma)?;
            if support::read_word(&mut dma, consumer)? == 1 {
                break;
            }
        }
        assert_eq!(support::read_word(&mut dma, consumer)?, 1);
        dma.drain_context(ContextId::new(7), &[id])?;
        assert_eq!(dma.context_status(ContextId::new(7)), Ok(false));
        // When: reset interrupts the affected Drain before execution finishes.
        dma.write(reset, &1_u32.to_le_bytes())?;
        // Then: Drain fails immediately, while reset stays busy until the continuation retires.
        assert_eq!(
            dma.context_status(ContextId::new(7)),
            Err(pdma_model_dma::MaintenanceError::ChannelStopped(id))
        );
        assert_eq!(
            dma.failed_context_channels(ContextId::new(7))
                .collect::<Vec<_>>(),
            [id]
        );
        assert_eq!(support::read_word(&mut dma, reset)?, 1);
        for _ in 0..128 {
            let _ = driver.step(&mut pipeline, &mut dma)?;
            if support::read_word(&mut dma, reset)? == 0 {
                break;
            }
        }
        assert_eq!(support::read_word(&mut dma, reset)?, 0);
        let mut destination = [0; 4];
        memory.read(0x1300.into(), MemoryAlignment::ONE, &mut destination)?;
        assert_eq!(destination, [0; 4]);
        assert_eq!(
            dma.context_status(ContextId::new(7)),
            Err(pdma_model_dma::MaintenanceError::ChannelStopped(id))
        );
    }
    Ok(())
}

#[test]
fn failed_drain_returns_channel_and_releases_reset_interlock() -> TestResult {
    // Given
    let (mut dma, memory) = setup(0x1100, 2)?;
    let mut input = command();
    input.source = pdma_model_contract::dma::Endpoint {
        context: 7,
        ..pdma_model_contract::dma::Endpoint::linear(0x9000, 0)
    };
    submit(&mut dma, &memory, input)?;
    let channel = ChannelId::new(0);
    let reset = channel.admin_offset(1).ok_or("layout")?;
    // When
    dma.drain_context(ContextId::new(7), &[channel])?;
    // Then
    assert_eq!(dma.write(reset, &0_u32.to_le_bytes()), Ok(()));
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..128 {
        let _ = operation.step(&mut pipeline, &mut dma)?;
    }
    assert_eq!(
        dma.context_status(ContextId::new(7)),
        Err(pdma_model_dma::MaintenanceError::ChannelStopped(channel))
    );
    dma.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..8 {
        let _ = operation.step(&mut pipeline, &mut dma)?;
    }
    assert_eq!(support::read_word(&mut dma, reset)?, 0);
    Ok(())
}
