//! Reset request, retirement and completion boundaries.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome, StepResult, Then};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, DmaControl, DmaRegisters,
    ResponseRegisters,
};
use pdma_model_dma::{Dma, Error};
use support::{TestResult, command, read_word, setup, submit};

const CHANNEL: usize =
    pdma_model_contract::dma::ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();
const RESET: usize = pdma_model_contract::dma::DmaDiscovery::EXTENT
    + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
const DEVICE_RESET: usize = (pdma_model_contract::dma::DmaDiscovery::EXTENT
    + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
    + DmaControl::RESET.offset();

struct CommandExit;

impl<M> Stage<pdma_model_dma::ExecutionContext<'_, M>> for CommandExit {
    type Input = pdma_model_dma::Flow<()>;
    type Output = pdma_model_dma::Flow<()>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut pdma_model_dma::ExecutionContext<'_, M>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        Ok(input.take().map_or(StepOutcome::InProgress, |flow| {
            StepOutcome::Complete(Ok(flow))
        }))
    }
}

#[test]
fn reset_invalidates_a_response_retained_outside_the_pipeline() -> TestResult {
    // Given: a fetched task retained between custom execution and response ports.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let input = support::accept(&mut device, ChannelId::new(0))?;
    let mut pipeline = Then::new(
        pdma_model_dma::PrepareChunk::default(),
        Then::new(
            pdma_model_dma::ReadCopy::default(),
            pdma_model_dma::WriteCopy::default(),
        ),
    );
    let work = {
        let mut context = device.execution_context();
        let mut operation = Operation::new(input);
        let mut output = None;
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut pipeline, &mut context)? {
                output = Some(result?);
                break;
            }
        }
        output.ok_or("missing completion")?
    };
    device.write(RESET, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut device, RESET)?, 1);
    let mut responses = device.responses();
    let mut publish = Some(work);
    // When: the final port rejects the old task before publishing a response.
    let mut rejected = false;
    for _ in 0..32 {
        if let StepOutcome::Complete(Ok(flow)) =
            responses.step(&mut device.execution_context(), &mut publish)?
        {
            assert_eq!(flow.status(), Err(Error::Reset));
            assert_eq!(read_word(&mut device, RESET)?, 1);
            assert_eq!(
                Operation::new(flow).step(&mut pdma_model_dma::Retire, &mut device)?,
                StepOutcome::Complete(Err(Error::Reset))
            );
            rejected = true;
            break;
        }
    }
    // Then: retirement cleans the owned command and the stale port cannot publish.
    assert!(rejected);
    assert_eq!(read_word(&mut device, RESET)?, 0);
    assert_eq!(
        read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    Ok(())
}

#[test]
fn reset_waits_for_the_resident_tail_after_response_publication() -> TestResult {
    // Given: a command whose response is published but whose composition has not exited.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let chunk = support::chunk_pipeline(&device);
    let pipeline = Then::new(
        chunk.upstream,
        Then::new(
            chunk.downstream.upstream,
            Then::new(chunk.downstream.downstream, CommandExit),
        ),
    );
    let mut hardware = device.hardware_with(pipeline);
    let command = &mut hardware;
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let producer = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
        + ChannelBlock::RESPONSE.offset()
        + ResponseRegisters::PRODUCER.offset();
    for _ in 0..128 {
        assert!(matches!(
            operation.step(command, &mut device)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut device, producer)? == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, producer)?, 1);
    // When: reset is requested before the complete command returns.
    device.write(RESET, &1_u32.to_le_bytes())?;
    // Then: the unfinished invocation keeps reset busy until its terminal step.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    let mut completed = false;
    for _ in 0..32 {
        assert!(matches!(
            operation.step(command, &mut device)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut device, RESET)? == 0 {
            completed = true;
            break;
        }
    }
    assert!(completed);
    assert_eq!(read_word(&mut device, producer)?, 0);
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn idle_reset_completes_without_a_hardware_step() -> TestResult {
    // Given: a configured idle channel without a resident invocation.
    let (mut device, _) = setup(0x1100, 2)?;
    // When: software requests reset, without stepping hardware yet.
    device.write(RESET, &1_u32.to_le_bytes())?;
    // Then: cleanup is already complete and configuration can be written again.
    assert_eq!(read_word(&mut device, RESET)?, 0);
    assert_eq!(
        read_word(&mut device, CHANNEL + ChannelRegisters::ENABLE.offset())?,
        0
    );
    device.write(
        ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
        &0x1000_u64.to_le_bytes(),
    )?;
    Ok(())
}

#[test]
fn reset_stays_busy_until_the_accepted_invocation_retires() -> TestResult {
    // Given: an accepted command whose execution state is still retained.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = device.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let consumer = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
        + ChannelBlock::COMMAND.offset()
        + CommandRegisters::CONSUMER.offset();
    for _ in 0..32 {
        let _ = operation.step(&mut hardware, &mut device)?;
        if read_word(&mut device, consumer)? == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, consumer)?, 1);
    device.write(RESET, &1_u32.to_le_bytes())?;
    // When: software attempts reconfiguration before the resident invocation retires.
    assert_eq!(
        device.write(
            CHANNEL + ChannelRegisters::ENABLE.offset(),
            &1_u32.to_le_bytes()
        ),
        Err(Error::ResetInProgress)
    );
    // Then: reset remains busy until the pipeline discards old work.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    for _ in 0..64 {
        let _ = operation.step(&mut hardware, &mut device)?;
        if read_word(&mut device, RESET)? == 0 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn repeated_and_zero_requests_do_not_cancel_or_complete_reset() -> TestResult {
    // Given: a reset request with an accepted invocation still resident.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = device.hardware();
    let mut driver = hardware_interface_pipeline::HardwareDriver::new();
    let consumer = ChannelBlock::ALIGNMENT
        + ChannelBlock::COMMAND.offset()
        + CommandRegisters::CONSUMER.offset();
    for _ in 0..64 {
        let _ = driver.step(&mut hardware, &mut device)?;
        if read_word(&mut device, consumer)? == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, consumer)?, 1);
    device.write(RESET, &1_u32.to_le_bytes())?;
    // When: software writes zero and repeats the request.
    device.write(RESET, &0_u32.to_le_bytes())?;
    device.write(RESET, &1_u32.to_le_bytes())?;
    // Then: busy remains asserted until the same invocation retires.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    for _ in 0..64 {
        let _ = driver.step(&mut hardware, &mut device)?;
        if read_word(&mut device, RESET)? == 0 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn idle_device_reset_preserves_discovery_and_clears_all_scopes() -> TestResult {
    // Given: three idle channels, one already reset.
    let mut device = Dma::new((), 3)?;
    let reset = ChannelId::new(1).admin_offset(3).ok_or("offset")?;
    device.write(reset, &1_u32.to_le_bytes())?;
    // When: software also requests device reset and tries to clear it.
    device.write(DEVICE_RESET, &1_u32.to_le_bytes())?;
    device.write(DEVICE_RESET, &0_u32.to_le_bytes())?;
    // Then: all scopes complete immediately and discovery remains available.
    for index in 0..3 {
        let address = ChannelId::new(index).admin_offset(3).ok_or("offset")?;
        let mut out = [0; 4];
        device.read(address, &mut out)?;
        assert_eq!(u32::from_le_bytes(out), 0);
    }
    let mut out = [0; 4];
    device.read(DEVICE_RESET, &mut out)?;
    assert_eq!(u32::from_le_bytes(out), 0);
    device.read(
        DmaRegisters::DISCOVERY.offset()
            + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset(),
        &mut out,
    )?;
    assert_eq!(u32::from_le_bytes(out), 3);
    Ok(())
}
