//! DMA integration with an independently owned flat byte store.

use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion,
    MemoryRegionError,
};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, DeviceRegisters, Endpoint, Response,
    ResponseRegisters, ResponseStatus,
};
use pdma_model_device::{Device, Pipeline, pipeline};
use std::{cell::RefCell, ops::Range};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
struct FlatMemory {
    bytes: RefCell<[u8; 512]>,
    extent: MemoryRegion,
}

impl FlatMemory {
    fn new() -> Result<Self, MemoryRegionError> {
        Ok(Self {
            bytes: RefCell::new([0; 512]),
            extent: MemoryRegion::try_new(MemoryAddress::new(0), 512, MemoryAlignment::ONE)?,
        })
    }

    fn range(address: MemoryAddress, length: usize) -> Result<Range<usize>, MemoryAccessError> {
        let error = MemoryAccessError::Unmapped { address };
        let start = usize::try_from(address.raw()).map_err(|_| error)?;
        let end = start.checked_add(length).ok_or(error)?;
        Ok(start..end)
    }
}

impl MemoryAccess for FlatMemory {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let end = address
            .checked_add(length)
            .ok_or(MemoryAccessError::AddressOverflow { address, length })?;
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        if address.raw() > 512 || (address.raw() == 512 && length != 0) {
            return Err(MemoryAccessError::Unmapped { address });
        }
        if end.raw() > 512 {
            return Err(MemoryAccessError::CrossesRegion {
                address,
                length,
                region: self.extent,
            });
        }
        Ok(())
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let length = u64::try_from(destination.len())
            .map_err(|_| MemoryAccessError::Unmapped { address })?;
        self.validate(address, alignment, length)?;
        let range = Self::range(address, destination.len())?;
        let bytes = self.bytes.borrow();
        destination.copy_from_slice(
            bytes
                .get(range)
                .ok_or(MemoryAccessError::Unmapped { address })?,
        );
        Ok(())
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let length =
            u64::try_from(source.len()).map_err(|_| MemoryAccessError::Unmapped { address })?;
        self.validate(address, alignment, length)?;
        let range = Self::range(address, source.len())?;
        self.bytes
            .borrow_mut()
            .get_mut(range)
            .ok_or(MemoryAccessError::Unmapped { address })?
            .copy_from_slice(source);
        Ok(())
    }
}

#[test]
fn flat_store_preserves_bytes_when_write_crosses_extent() -> TestResult {
    // Given
    let memory = FlatMemory::new()?;
    memory.write(MemoryAddress::new(510), MemoryAlignment::ONE, &[5, 6])?;
    // When
    let result = memory.write(MemoryAddress::new(511), MemoryAlignment::ONE, &[9, 9]);
    // Then
    assert!(matches!(
        result,
        Err(MemoryAccessError::CrossesRegion { .. })
    ));
    let mut bytes = [0; 2];
    memory.read(MemoryAddress::new(510), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [5, 6]);
    assert_eq!(
        memory.validate(MemoryAddress::new(512), MemoryAlignment::ONE, 0),
        Ok(())
    );
    Ok(())
}

#[test]
fn pipeline_copies_with_an_independent_flat_store() -> TestResult {
    // Given
    let mut device = Device::new(FlatMemory::new()?);
    let command = Command {
        id: 91,
        source: Endpoint::linear(256, 4),
        destination: Endpoint::linear(320, 4),
    };
    device.memory.write(
        MemoryAddress::new(0),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    device
        .memory
        .write(MemoryAddress::new(256), MemoryAlignment::ONE, &[8, 6, 7, 5])?;
    device.write(
        DeviceRegisters::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
        &128_u64.to_le_bytes(),
    )?;
    for offset in [
        DeviceRegisters::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
        DeviceRegisters::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::ENABLE.offset(),
        DeviceRegisters::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
    ] {
        device.write(offset, &1_u32.to_le_bytes())?;
    }
    let mut hardware = pipeline();
    let mut operation = Operation::<Pipeline, Device<FlatMemory>>::new(());
    let mut completed = false;
    // When
    for _ in 0..32 {
        match StepExecutor::new()
            .step(&mut hardware, &mut operation, &mut device)
            .map_err(|error| format!("{error:?}"))?
        {
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
            RunOutcome::Complete(result) => {
                result?;
                completed = true;
                break;
            }
        }
    }
    // Then
    assert!(completed);
    let mut destination = [0; 4];
    device.memory.read(
        MemoryAddress::new(320),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [8, 6, 7, 5]);
    let mut response = [0; Response::RECORD_SIZE];
    device
        .memory
        .read(MemoryAddress::new(128), MemoryAlignment::ONE, &mut response)?;
    assert_eq!(
        Response::from_bytes(&response)?,
        Response {
            id: 91,
            status: ResponseStatus::Success,
            bytes_completed: 4,
        }
    );
    assert!(device.irq());
    Ok(())
}
