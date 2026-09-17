use hardware_interface_runtime::{
    MemoryAccess, MemoryAlignment, MemoryRegion, SharedMemory, SoftwareCpu,
};
use hardware_interface_schema::record_schema;
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing, SoftRingError, SoftRingIo};

record_schema! {
    #[short_description = "Validated entry"]
    #[description = "Test wire value with an invalid discriminant."]
    record Tagged {
        alignment: 4, endian: Little,
        fields: {
            #[description = "Only zero is valid."]
            tag: enum(u32) { variants: { Valid = 0 } },
        },
    }
}

#[tokio::test]
async fn malformed_entry_is_not_consumed() -> Result<(), Box<dyn std::error::Error>> {
    // Given: one published entry with an unassigned discriminant.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    memory.write(16.into(), MemoryAlignment::ONE, &7u32.to_le_bytes())?;
    io.write_producer(1).await?;
    SoftwareCpu::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Tagged>::new(16, 2)?)?;
            // When: decoding fails and the caller tries to consume.
            let decoded = ring.peek(&io).await;
            let consumed = ring.consume(&io).await;
            // Then: malformed data never creates a consumable cache or cursor update.
            assert!(matches!(decoded, Err(SoftRingError::Codec(_))));
            assert!(matches!(consumed, Err(SoftRingError::Idle)));
            assert_eq!(io.read_consumer().await?, 0);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}
