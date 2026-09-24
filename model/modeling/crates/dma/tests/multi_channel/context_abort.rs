use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandRegisters, CompletionFlagsValues, ContextId, Endpoint,
    ResponseRegisters,
};
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
        let target = Command {
            id: 7,
            flags: CompletionFlagsValues::default(),
            sg_context: 0,
            source: Endpoint {
                context: 7,
                ..Endpoint::linear(0x1200, 512)
            },
            destination: Endpoint::linear(0x1400, 512),
        };
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
        let mut pipeline = dma.command_loop();
        let mut operation = Operation::new(());
        for _ in 0..steps {
            let _ = operation.step(&mut pipeline, &mut dma);
        }
        if word(&mut dma, first + CommandRegisters::CONSUMER.offset())? != 0 {
            continue;
        }
        checked += 1;
        // When
        dma.abort_context(ContextId::new(7), &[ChannelId::new(0), ChannelId::new(1)])?;
        for _ in 0..256 {
            assert!(matches!(
                operation.step(&mut pipeline, &mut dma),
                StepOutcome::Progressed | StepOutcome::Pending(())
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
                peer + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
            )?,
            1,
            "Abort after {steps} unaccepted steps stranded the peer"
        );
        assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    }
    assert!(checked > 1);
    Ok(())
}
