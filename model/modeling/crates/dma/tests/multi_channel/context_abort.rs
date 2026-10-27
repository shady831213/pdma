use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandRegisters, ContextId, CopyCommandDestinationValues,
    CopyCommandSourceValues, EventRegisters, LinearEndpoint,
};

use super::support::{TestResult, completion, configure, submit, word};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_pipeline::StepOutcome;
use pdma_model_dma::Dma;

#[test]
fn abort_during_unaccepted_arbitration_does_not_stall_peer_channels() -> TestResult {
    // Given
    let mut checked = 0;
    for steps in 0..12 {
        let memory = SharedMemory::new();
        memory.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x3000,
            MemoryAlignment::ONE,
        )?])?;
        let mut dma = Dma::new(memory.clone(), 2)?;
        configure(&mut dma, 0)?;
        configure(&mut dma, 1)?;
        let target = Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 7,
            copy_length: 512,
            sg_context: 0,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x1200,
                offset: 0,
            }),
            source_context: 7,
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1400,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        });
        memory.write(
            MemoryAddress::new(0x1000),
            MemoryAlignment::ONE,
            &target.to_bytes()?,
        )?;
        let first = ChannelId::new(0).register_offset(2).ok_or("channel")?;
        let peer = ChannelId::new(1).register_offset(2).ok_or("peer")?;
        dma.write(
            first + CommandRegisters::PRODUCER.offset(),
            &1_u32.to_le_bytes(),
        )?;
        submit(&mut dma, 1, 8)?;
        let mut pipeline = dma.hardware();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        for _ in 0..steps {
            let _ = operation.step(&mut pipeline, &mut dma)?;
        }
        if word(&mut dma, first + CommandRegisters::CONSUMER.offset())? != 0 {
            continue;
        }
        checked += 1;
        // When
        dma.abort_context(ContextId::new(7), &[ChannelId::new(0), ChannelId::new(1)])?;
        for _ in 0..256 {
            assert!(matches!(
                operation.step(&mut pipeline, &mut dma)?,
                StepOutcome::InProgress
            ));
        }
        // Then
        assert_eq!(
            word(&mut dma, first + CommandRegisters::CONSUMER.offset())?,
            1
        );
        assert_eq!(
            word(
                &mut dma,
                peer + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset()
            )?,
            0,
            "Abort after {steps} unaccepted steps stranded the peer"
        );
        assert_eq!(completion(&dma, 1)?, 9);
        assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    }
    assert!(checked > 1);
    Ok(())
}
