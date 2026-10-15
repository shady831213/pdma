//! Reset request, retirement and completion boundaries.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome, StepResult, Then};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, DmaControl, DmaRegisters,
    ResponseRegisters,
};
use pdma_model_dma::{Dma, Error, ResetControl};
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
    type Input = ();
    type Output = ();
    type Error = pdma_model_dma::ExecutionError;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        _: &mut pdma_model_dma::ExecutionContext<'_, M>,
        (): &mut (),
    ) -> StepResult<(), pdma_model_dma::ExecutionError> {
        Ok(StepOutcome::Complete(Ok(())))
    }
}

#[test]
fn rejected_response_port_does_not_retire_the_command() -> TestResult {
    // Given: a fetched task retained between custom execution and response ports.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let input = support::accept(&mut device, ChannelId::new(0))?;
    let mut pipeline = Then::new(
        pdma_model_dma::PrepareChunk,
        Then::new(pdma_model_dma::ReadCopy, pdma_model_dma::WriteCopy),
    );
    let work = {
        let mut context = device.execution_context(ChannelId::new(0))?;
        let mut operation = Operation::new(Some(input));
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
    let mut responses = device.responses();
    let mut publish = <pdma_model_dma::Responses as Stage<
        pdma_model_dma::ExecutionContext<'_, hardware_interface_runtime::SharedMemory>,
    >>::start(work);
    // When: the final port rejects the old task before publishing a response.
    let mut rejected = false;
    for _ in 0..32 {
        if matches!(
            responses.step(
                &mut device.execution_context(ChannelId::new(0))?,
                &mut publish
            )?,
            StepOutcome::Complete(Err(pdma_model_dma::ExecutionError::Rejected(Error::Reset)))
        ) {
            rejected = true;
            break;
        }
    }
    // Then: response rejection alone cannot acknowledge whole-command cleanup.
    assert!(rejected);
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 1);
    Ok(())
}

#[test]
fn published_response_retires_hardware_before_custom_command_exit() -> TestResult {
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
    let mut hardware = device.service_with(pipeline);
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
    let _ = ResetControl.step(&mut device, &mut ())?;
    // Then: response publication has released hardware ownership before the generic tail exits.
    assert_eq!(read_word(&mut device, RESET)?, 0);
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
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn idle_reset_is_a_request_until_hardware_steps() -> TestResult {
    // Given: a configured idle channel and retained hardware loop.
    let (mut device, _) = setup(0x1100, 2)?;
    let mut hardware = device.service();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    // When: software requests reset, without stepping hardware yet.
    device.write(RESET, &1_u32.to_le_bytes())?;
    // Then: busy is visible and reconfiguration is rejected until hardware cleanup.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert_eq!(
        device.write(
            CHANNEL + ChannelRegisters::ENABLE.offset(),
            &1_u32.to_le_bytes()
        ),
        Err(Error::ResetInProgress)
    );
    let _ = operation.step(&mut hardware, &mut device)?;
    assert_eq!(read_word(&mut device, RESET)?, 0);
    assert_eq!(
        read_word(&mut device, CHANNEL + ChannelRegisters::ENABLE.offset())?,
        0
    );
    Ok(())
}

#[test]
fn reset_control_cannot_acknowledge_accepted_work_before_retirement() -> TestResult {
    // Given: an accepted command whose execution state is still retained.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = device.service();
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
    // When: only the reset controller advances, without retiring the command.
    assert!(matches!(
        ResetControl.step(&mut device, &mut ())?,
        StepOutcome::Complete(Ok(()))
    ));
    // Then: reset remains busy until the retained command loop discards old work.
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
    // Given: a reset request awaiting its first hardware step.
    let (mut device, _) = setup(0x1100, 2)?;
    device.write(RESET, &1_u32.to_le_bytes())?;
    // When: software writes zero and repeats the request.
    device.write(RESET, &0_u32.to_le_bytes())?;
    device.write(RESET, &1_u32.to_le_bytes())?;
    // Then: busy remains asserted; one idle cleanup completes the coalesced request.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}

#[test]
fn device_reset_subsumes_channel_reset_and_preserves_discovery() -> TestResult {
    // Given: three idle channels, one with a pending channel reset.
    let mut device = Dma::new((), 3)?;
    let reset = ChannelId::new(1).admin_offset(3).ok_or("offset")?;
    device.write(reset, &1_u32.to_le_bytes())?;
    // When: software also requests device reset and tries to clear it.
    device.write(DEVICE_RESET, &1_u32.to_le_bytes())?;
    device.write(DEVICE_RESET, &0_u32.to_le_bytes())?;
    // Then: all scopes remain busy until hardware completes the device reset.
    for index in 0..3 {
        let address = ChannelId::new(index).admin_offset(3).ok_or("offset")?;
        let mut out = [0; 4];
        device.read(address, &mut out)?;
        assert_eq!(u32::from_le_bytes(out), 1);
    }
    let _ = ResetControl.step(&mut device, &mut ())?;
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
