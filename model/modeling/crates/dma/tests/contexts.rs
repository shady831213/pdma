//! Independent metadata and payload address spaces.

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    Endpoint, Response, ResponseRegisters, ResponseStatus, Segment,
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
    let command = Command {
        id: 7,
        flags: CompletionFlagsValues::default(),
        ..Command::copy(
            0,
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
        )
    };
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
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
            0x1100,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    for offset in [
        ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
        ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
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
    let mut response = [0; Response::RECORD_SIZE];
    dma.memory.read_with_sideband(
        &4,
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
        &mut response,
    )?;
    assert_eq!(
        Response::from_bytes(&response)?.status,
        ResponseStatus::Success
    );
    Ok(())
}
