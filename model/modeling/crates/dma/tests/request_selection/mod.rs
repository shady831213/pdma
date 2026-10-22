use super::*;
use pdma_model_dma::{CheckRanges, PrepareChunk, ReadCopy};

#[test]
fn active_task_is_not_readmitted_and_backpressures_peers() -> TestResult {
    // Given: a selected descriptor already accepted by Fetch.
    let mut device = fixture()?;
    let selected = ChannelId::new(0);
    let Some(CommandRequest::Fetch(snapshot)) = device.command_request(selected)? else {
        return Err("fresh snapshot".into());
    };
    assert!(matches!(
        Operation::new(Some(CommandRequest::Fetch(snapshot)))
            .step(&mut Fetch, &mut device)?,
        StepOutcome::Complete(Ok(ref flow)) if flow.data().is_some()
    ));
    // When: admission samples candidates while command execution retains its grant.
    let continuation = device.command_request(selected)?;
    let peer = device.command_request(ChannelId::new(1))?;
    // Then: neither the active command nor its peer can be admitted again.
    assert!(continuation.is_none());
    assert!(peer.is_none());
    Ok(())
}

#[test]
fn stale_fetch_cannot_substitute_a_reconfigured_head() -> TestResult {
    // Given: a snapshot taken before a reset and reconfiguration.
    let mut device = fixture()?;
    let selected = ChannelId::new(0);
    let Some(CommandRequest::Fetch(snapshot)) = device.command_request(selected)? else {
        return Err("fresh snapshot".into());
    };
    let base = selected.register_offset(2).ok_or("channel")?;
    device.write(
        pdma_model_contract::dma::DmaDiscovery::EXTENT
            + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET,
        &1_u32.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
        &0x1100_u64.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
        &2_u32.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
        &0x1800_u64.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
        &2_u32.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    device.write(
        base + ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    // When: the old selected request arrives after reconfiguration.
    let result =
        Operation::new(Some(CommandRequest::Fetch(snapshot))).step(&mut Fetch, &mut device)?;
    // Then: the stale identity cannot read or accept the new head.
    assert!(!matches!(result, StepOutcome::Complete(Ok(ref flow)) if flow.data().is_some()));
    assert!(device.memory.reads.borrow().is_empty());
    Ok(())
}

#[test]
fn active_command_candidate_sweeps_preserve_round_robin_position() -> TestResult {
    // Given: a fresh grant rotates the next winner to the second source.
    let mut device = fixture()?;
    let fresh = [
        Some(device.command_request(ChannelId::new(0))?),
        Some(device.command_request(ChannelId::new(1))?),
    ];
    let mut policy = RoundRobin::default();
    assert_eq!(policy.select(&fresh)?, Some(0));
    let Some(Some(CommandRequest::Fetch(snapshot))) = fresh[0] else {
        return Err("fresh snapshot".into());
    };
    assert!(matches!(
        Operation::new(Some(CommandRequest::Fetch(snapshot))).step(
            &mut Fetch,
            &mut device
        )?,
        StepOutcome::Complete(Ok(ref flow)) if flow.data().is_some()
    ));
    // When: repeated candidate sweeps occur while the accepted command is active.
    for _ in 0..4 {
        let candidates = [
            Some(device.command_request(ChannelId::new(0))?),
            Some(device.command_request(ChannelId::new(1))?),
        ];
        assert_eq!(policy.select(&candidates)?, None);
    }
    // Then: empty sweeps have not rotated the next fresh winner.
    assert_eq!(policy.select(&fresh)?, Some(1));
    Ok(())
}

#[test]
fn busy_read_retains_its_chunk_and_leaves_the_next_offer_untouched() -> TestResult {
    // Given: a concrete chunk whose owner is disabled after preparation.
    let mut device = fixture()?;
    let id = ChannelId::new(0);
    let Some(CommandRequest::Fetch(snapshot)) = device.command_request(id)? else {
        return Err("fresh snapshot".into());
    };
    let StepOutcome::Complete(Ok(fetched)) =
        Operation::new(Some(CommandRequest::Fetch(snapshot))).step(&mut Fetch, &mut device)?
    else {
        return Err("fetch completion".into());
    };
    let mut context = device.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, fetched)?;
    let request = context.install_progress(progress);
    let chunk = finish(&mut PrepareChunk::default(), &mut context, request)?;
    let enable = id.register_offset(2).ok_or("channel")?
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::ENABLE.offset();
    device.write(enable, &0_u32.to_le_bytes())?;
    let mut read = ReadCopy::default();
    let mut input = Some(chunk);
    assert!(matches!(
        read.step(&mut device.execution_context(), &mut input)?,
        StepOutcome::InProgress
    ));
    assert!(input.is_none());
    // When: another offer arrives while the stage retains the first chunk.
    let bubble = finish(&mut Fetch, &mut device, None)?;
    let mut context = device.execution_context();
    let progress = finish(&mut CheckRanges::default(), &mut context, bubble)?;
    let request = context.install_progress(progress);
    let mut next = Some(finish(&mut PrepareChunk::default(), &mut context, request)?);
    assert!(matches!(
        read.step(&mut device.execution_context(), &mut next)?,
        StepOutcome::InProgress
    ));
    device.write(enable, &1_u32.to_le_bytes())?;
    let StepOutcome::Complete(Ok(payload)) =
        read.step(&mut device.execution_context(), &mut next)?
    else {
        return Err("retained chunk was lost".into());
    };
    // Then: only the original chunk is read and the next offer is still pending.
    assert_eq!(
        payload.data().ok_or("payload")?.chunk().source(),
        MemoryAddress::new(0x2000)
    );
    assert_eq!(payload.data().ok_or("payload")?.bytes()?, &[0; 4]);
    assert!(next.as_ref().is_some_and(|flow| flow.data().is_none()));
    assert_eq!(device.memory.reads.borrow().as_slice(), &[0x1000, 0x2000]);
    Ok(())
}

fn finish<C, S: Stage<C>>(stage: &mut S, context: &mut C, input: S::Input) -> TestResult<S::Output>
where
    S::Error: std::error::Error + 'static,
{
    let mut operation = Operation::new(input);
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(stage, context)? {
            return Ok(result?);
        }
    }
    Err("stage watchdog".into())
}

#[test]
fn buffered_command_observes_reset_before_validation_or_payload_access() -> TestResult {
    // Given: a connected graph that has actually fetched its selected descriptor.
    let mut device = fixture()?;
    let mut hardware = device.hardware();
    let mut operation = Operation::new(());
    for _ in 0..128 {
        let _ = operation.step(&mut hardware, &mut device)?;
        if device.memory.reads.borrow().contains(&0x1000) {
            break;
        }
    }
    assert!(device.memory.reads.borrow().contains(&0x1000));
    // When: MMIO requests cleanup while the fetched command is buffered in Then.
    let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    let mut busy = [0; 4];
    device.read(reset, &mut busy)?;
    assert_eq!(u32::from_le_bytes(busy), 1);
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
            break;
        }
    }
    // Then: resident invocation ownership cannot leave reset permanently busy.
    device.read(reset, &mut busy)?;
    assert_eq!(u32::from_le_bytes(busy), 0);
    assert_eq!(device.memory.reads.borrow().as_slice(), &[0x1000]);
    Ok(())
}
