//! Independent metadata and payload address spaces.

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Endpoint, EventRegisters,
    Segment,
};
use pdma_model_dma::Dma;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
struct Spaces(Vec<SharedMemory>);

impl Spaces {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut spaces = Vec::new();
        for _ in 0..5 {
            let memory = SharedMemory::new();
            memory.add_regions(&[MemoryRegion::try_new(
                MemoryAddress::new(0x1000),
                4096,
                MemoryAlignment::ONE,
            )?])?;
            spaces.push(memory);
        }
        Ok(Self(spaces))
    }

    fn get(
        &self,
        context: u32,
        address: MemoryAddress,
    ) -> Result<&SharedMemory, MemoryAccessError> {
        self.0
            .get(usize::try_from(context).map_err(|_| MemoryAccessError::Unmapped { address })?)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

impl MemoryAccess<u32> for Spaces {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        context: &u32,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.get(*context, address)?
            .validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        context: &u32,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*context, address)?.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        context: &u32,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*context, address)?
            .write(address, alignment, bytes)
    }
}

#[test]
fn identical_addresses_use_independent_ring_sg_source_and_destination_spaces() -> TestResult {
    // Given
    let mut dma = Dma::with_sideband(Spaces::new()?, 1, |access| access.context.raw())?;
    let command = Command::copy(
        7,
        pdma_model_contract::dma::CopyCommand {
            copy_length: 4,
            sg_context: 1,
            source: Endpoint {
                context: 2,
                ..Endpoint::scatter_gather(0x1200, 1)
            },
            destination: Endpoint {
                context: 3,
                ..Endpoint::scatter_gather(0x1200, 1)
            },
        },
    );
    dma.memory.write_with_sideband(
        &4,
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    dma.memory.write_with_sideband(
        &1,
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &Segment {
            address: 0x1300,
            length: 4,
        }
        .to_bytes()?,
    )?;
    dma.memory.write_with_sideband(
        &2,
        MemoryAddress::new(0x1300),
        MemoryAlignment::ONE,
        &[8, 6, 7, 5],
    )?;
    let base = ChannelId::new(0).register_offset(1).ok_or("layout")?;
    dma.write(
        base + ChannelBlock::CONTROL.offset() + ChannelRegisters::RING_CONTEXT.offset(),
        &4_u32.to_le_bytes(),
    )?;
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
            0x1100,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    for offset in [
        ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
        ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
        ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
        ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
    ] {
        dma.write(base + offset, &1_u32.to_le_bytes())?;
    }
    let mut pipeline = dma.hardware();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    // When
    for _ in 0..128 {
        let outcome = operation
            .step(&mut pipeline, &mut dma)
            .map_err(|e| format!("{e:?}"))?;
        assert!(!matches!(
            outcome,
            hardware_interface_pipeline::StepOutcome::Complete(_)
        ));
    }
    // Then
    let mut bytes = [0; 4];
    dma.memory.read_with_sideband(
        &3,
        MemoryAddress::new(0x1300),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    assert_eq!(bytes, [8, 6, 7, 5]);
    let mut producer = [0; 4];
    dma.read(
        base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset(),
        &mut producer,
    )?;
    assert_eq!(u32::from_le_bytes(producer), 0);
    Ok(())
}

#[test]
fn timestamp_uses_target_context_not_ring_or_default_context() -> TestResult {
    use pdma_model_contract::dma::{Event, EventCode, TimestampCommand};
    #[derive(Debug)]
    struct ExternalTimer(u64);
    impl pdma_model_dma::Timer for ExternalTimer {
        fn sample(&self) -> u64 {
            self.0
        }
    }
    // Given: identical virtual addresses in independent spaces; metadata uses context four.
    let sample = 0x1234_5678_9abc_def0_u64;
    for target in [1, 7] {
        let mut dma = Dma::with_sideband(Spaces::new()?, 1, |access| access.context.raw())?
            .with_timer(std::sync::Arc::new(ExternalTimer(sample)));
        let command = Command::timestamp(
            7,
            TimestampCommand {
                address: 0x1300,
                context: target,
            },
        );
        dma.memory.write_with_sideband(
            &4,
            0x1000.into(),
            MemoryAlignment::ONE,
            &command.to_bytes()?,
        )?;
        let base = ChannelId::new(0).register_offset(1).ok_or("layout")?;
        dma.write(
            base + ChannelBlock::CONTROL.offset() + ChannelRegisters::RING_CONTEXT.offset(),
            &4_u32.to_le_bytes(),
        )?;
        for (offset, address) in [
            (
                ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                0x1000_u64,
            ),
            (
                ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                0x1100,
            ),
        ] {
            dma.write(base + offset, &address.to_le_bytes())?;
        }
        for offset in [
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
        ] {
            dma.write(base + offset, &1_u32.to_le_bytes())?;
        }
        let mut hardware = dma.hardware();
        let mut driver = hardware_interface_pipeline::HardwareDriver::new();
        // When: Timestamp writes using the command's selector, or that selector is rejected.
        for _ in 0..128 {
            let _ = driver.step(&mut hardware, &mut dma)?;
        }
        // Then: no fallback to ring/default context can hide an invalid target selector.
        for context in 0..5 {
            let mut bytes = [0; 8];
            dma.memory.read_with_sideband(
                &context,
                0x1300.into(),
                MemoryAlignment::ONE,
                &mut bytes,
            )?;
            assert_eq!(
                u64::from_le_bytes(bytes),
                if context == target { sample } else { 0 }
            );
        }
        let mut producer = [0; 4];
        dma.read(
            base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset(),
            &mut producer,
        )?;
        assert_eq!(u32::from_le_bytes(producer), u32::from(target == 7));
        if target == 7 {
            let mut bytes = vec![0; Event::RECORD_SIZE];
            dma.memory
                .read_with_sideband(&4, 0x1100.into(), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(Event::from_bytes(&bytes)?.code, EventCode::WriteError);
        }
    }
    Ok(())
}
