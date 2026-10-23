use super::{FaultMemory, rig};
use crate::command_operations::support::{EVENT_PRODUCER, Rig, TestResult, fence};
use pdma_model_contract::dma::{ChannelBlock, ChannelRegisters, Command, TimestampCommand};
use pdma_model_dma::Timer;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
mod lifecycle;

#[derive(Debug)]
struct PlatformTimer(AtomicU64);

impl Timer for PlatformTimer {
    fn sample(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

const fn timestamp(id: u64, address: u64) -> Command {
    Command::timestamp(
        id,
        TimestampCommand {
            address,
            context: 0,
        },
    )
}

fn resume(rig: &mut Rig<FaultMemory>) -> TestResult {
    rig.dma.write(
        rig.bases.first().ok_or("channel missing")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

#[test]
fn timestamp_retains_first_sample_across_repeated_write_faults() -> TestResult {
    // Given: a timestamp write faults; the external clock is independent of DMA execution.
    let mut rig = rig(0x9000)?;
    let timer = Arc::new(PlatformTimer(AtomicU64::new(17)));
    rig.dma = rig.dma.with_timer(timer.clone());
    rig.submit(
        0,
        &[timestamp(1, 0x9000), timestamp(2, 0x9010), fence(0x8000)],
    )?;
    // When: the clock changes during suspension and one premature resume faults again.
    rig.drive()?;
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    timer.0.store(81, Ordering::Relaxed);
    resume(&mut rig)?;
    rig.drive()?;
    assert_eq!(rig.word(0x8000)?, 0);
    rig.dma.memory.fault.set(false);
    resume(&mut rig)?;
    rig.drive()?;
    // Then: the first command writes its original sample; only the next samples the new time.
    assert_eq!(rig.word(0x9000)?, 17);
    assert_eq!(rig.word(0x9010)?, 81);
    assert_eq!(rig.word(0x8000)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert!(!rig.dma.take_interrupt());
    Ok(())
}

#[test]
fn zero_is_a_retained_sample_not_an_unsampled_marker() -> TestResult {
    // Given: zero is a legitimate external timer value.
    let mut rig = rig(0x9000)?;
    let timer = Arc::new(PlatformTimer(AtomicU64::new(0)));
    rig.dma = rig.dma.with_timer(timer.clone());
    rig.submit(0, &[timestamp(1, 0x9000), fence(0x8000)])?;
    // When: the write faults and the external timer changes before repair.
    rig.drive()?;
    timer.0.store(u64::MAX, Ordering::Relaxed);
    rig.dma.memory.fault.set(false);
    resume(&mut rig)?;
    rig.drive()?;
    // Then: publication keeps the zero sample and completes normally.
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.word(0x8000)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn timestamp_samples_execution_time_not_fetch_time() -> TestResult {
    use pdma_model_contract::dma::CommandRegisters;
    // Given: software publishes a command while the clock holds its old value.
    let mut rig = rig(0xa000)?;
    let timer = Arc::new(PlatformTimer(AtomicU64::new(17)));
    rig.dma = rig.dma.with_timer(timer.clone());
    rig.submit(0, &[timestamp(1, 0x9000), fence(0x8000)])?;
    let consumer = ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset();
    // When: the timer advances after fetch but before the execution branch.
    for _ in 0..256 {
        rig.step()?;
        if rig.register(0, consumer)? == 1 {
            break;
        }
    }
    assert_eq!(rig.register(0, consumer)?, 1);
    assert_eq!(rig.word(0x9000)?, 0);
    timer.0.store(81, Ordering::Relaxed);
    rig.drive()?;
    // Then: the stored sample comes from execution, not admission or fetch.
    assert_eq!(rig.word(0x9000)?, 81);
    assert_eq!(rig.word(0x8000)?, 0xfeed);
    Ok(())
}

#[test]
fn timestamp_rejections_stop_without_publishing_success() -> TestResult {
    use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
    use pdma_model_contract::dma::{Event, EventCode};
    // Given: absent source, invalid destination geometry, or an inaccessible target.
    for (address, source, code) in [
        (0x9000, false, EventCode::Unsupported),
        (0x9001, true, EventCode::InvalidOperation),
        (u64::MAX - 7, true, EventCode::InvalidOperation),
        (0x10000, true, EventCode::WriteError),
    ] {
        let mut rig = rig(0xa000)?;
        if source {
            rig.dma = rig
                .dma
                .with_timer(Arc::new(PlatformTimer(AtomicU64::new(17))));
        }
        rig.submit(0, &[timestamp(1, address), fence(0x8000)])?;
        // When: execution rejects the command through the ordinary stopping-error path.
        rig.drive()?;
        // Then: the event identifies the request and no successor completion is published.
        let mut bytes = vec![0; Event::RECORD_SIZE];
        rig.memory
            .read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
        let event = Event::from_bytes(&bytes)?;
        assert_eq!((event.id, event.code), (1, code));
        assert_eq!(rig.word(0x8000)?, 0);
        assert_eq!(rig.register(0, EVENT_PRODUCER)?, 1);
    }
    Ok(())
}

#[test]
fn fence_access_error_still_stops_instead_of_parking() -> TestResult {
    use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
    use pdma_model_contract::dma::{Event, EventCode};
    // Given: a Fence destination outside the backing memory, not a repairable fault.
    let mut rig = rig(0xa000)?;
    rig.submit(0, &[fence(0x10000), timestamp(2, 0x9000)])?;
    // When: the Fence encounters the permanent access error.
    rig.drive()?;
    // Then: it reports WriteError and never marks a resumable fault.
    let mut bytes = vec![0; Event::RECORD_SIZE];
    rig.memory
        .read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(Event::from_bytes(&bytes)?.code, EventCode::WriteError);
    assert_eq!(
        rig.register(
            0,
            ChannelBlock::CONTROL.offset() + ChannelRegisters::FAULT_STATUS.offset()
        )?,
        0
    );
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 1);
    Ok(())
}
