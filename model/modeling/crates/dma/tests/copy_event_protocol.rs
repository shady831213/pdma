//! Successful commands never consume error-reporting capacity.
#[doc = "Shared hardware test fixtures."]
pub mod support;

use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_pipeline::HardwareDriver;
use pdma_model_contract::dma::{ChannelBlock, EventRegisters};
use support::{TestResult, command, read_word, setup, submit};

#[test]
fn successful_copy_leaves_event_producer_unchanged() -> TestResult {
    // Given: one copy and empty error-reporting storage.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[1, 2, 3, 4],
    )?;
    submit(&mut device, &memory, command())?;
    let mut hardware = device.hardware();
    let mut driver = HardwareDriver::new();
    // When: hardware drains the command.
    for _ in 0..128 {
        let _ = driver.step(&mut hardware, &mut device)?;
    }
    // Then: payload commits without any success event.
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [1, 2, 3, 4]);
    assert_eq!(
        read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );
    Ok(())
}
