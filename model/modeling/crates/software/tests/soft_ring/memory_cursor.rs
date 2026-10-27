use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use pdma_model_software::ring::{MemoryRingIo, SoftRingIo};

#[tokio::test]
async fn cursor_accesses_fit_separate_four_byte_regions() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: independent four-byte regions prohibit whole-header accesses.
    let memory = SharedMemory::new();
    memory.add_regions(&[
        MemoryRegion::try_new(0.into(), 4, MemoryAlignment::ONE)?,
        MemoryRegion::try_new(4.into(), 4, MemoryAlignment::ONE)?,
    ])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    // When: both fields are published through their borrowed memory adapter.
    io.write_consumer(0x1020_3040).await?;
    io.write_producer(0x5060_7080).await?;
    // Then: exact-width little-endian accesses preserve the other field.
    assert_eq!(io.read_consumer().await?, 0x1020_3040);
    assert_eq!(io.read_producer().await?, 0x5060_7080);
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?,
        [0x80, 0x70, 0x60, 0x50]
    );
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(4.into(), MemoryAlignment::ONE)?,
        [0x40, 0x30, 0x20, 0x10]
    );
    Ok(())
}
