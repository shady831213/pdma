use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_schema::record_schema;
use hardware_interface_software::SoftwareEnv;
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
    SoftwareEnv::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Tagged>::new(16, 2)?);
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

#[tokio::test]
async fn rejected_command_encoding_does_not_block_later_publication()
-> Result<(), Box<dyn std::error::Error>> {
    use pdma_model_contract::dma::{Command, CommandHeader, Opcode, TrapCommand};

    // Given: valid storage and an application command with a mismatched header.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 256, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    let mut ring = SoftRing::new(RingConfig::<Command>::new(16, 2)?);
    let malformed = Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Copy,
        },
        id: 17,
    });
    let valid = Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: 18,
    });
    // When: software submits an entry whose encoding is rejected before I/O.
    let rejected = ring.try_push(&io, malformed).await;
    // Then: nothing is published or retained, and the corrected command can proceed.
    assert!(matches!(rejected, Err(SoftRingError::Codec(_))));
    assert_eq!(io.read_producer().await?, 0);
    let mut storage = [0xff; Command::RECORD_SIZE];
    memory.read(16.into(), MemoryAlignment::ONE, &mut storage)?;
    assert_eq!(storage, [0; Command::RECORD_SIZE]);
    assert!(!ring.has_pending_push());
    assert!(ring.try_push(&io, valid).await?);
    assert_eq!(ring.peek(&io).await?, Some(valid));
    Ok(())
}
