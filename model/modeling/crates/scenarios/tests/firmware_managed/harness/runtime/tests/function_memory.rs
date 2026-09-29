//! Scenario memory routing through the authoritative Function owner bank.
use crate::harness::runtime::memory::{RouteError, RoutedMemory, SpaceError};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};
use pdma_model_dma::Dma;
use pdma_model_hw_components::windows::ChannelWindows;
use std::sync::{Arc, Mutex};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const WINDOWS: usize = 0x10_0000;

#[test]
fn closed_window_retains_memory_binding_until_reset_and_reassignment() -> TestResult {
    // Given: two backends map identical addresses, and channel zero belongs to Function one.
    let first = space([0x11; 4])?;
    let second = space([0x22; 4])?;
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(WINDOWS)?)?;
    windows.management_write(
        WINDOWS + WindowControl::OWNER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    windows.management_write(WINDOWS, &1_u32.to_le_bytes())?;
    let windows = Arc::new(Mutex::new(windows));
    let mut dma = Dma::with_sideband(
        RoutedMemory::new(Arc::clone(&windows), vec![first.clone(), second.clone()])?,
        1,
        |access| access,
    )?;
    // When: firmware closes Host MMIO before draining old DMA work.
    windows
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .management_write(WINDOWS, &0_u32.to_le_bytes())?;
    let context = dma.channel_context(ChannelId::new(0))?;
    let bytes = context
        .memory
        .read_typed::<[u8; 4]>(MemoryAddress::new(0x1000), MemoryAlignment::ONE)?;
    context
        .memory
        .write_typed(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &[0x33; 4])?;
    // Then: closure neither revokes the old memory port nor falls back to Function zero.
    assert_eq!(bytes, [0x22; 4]);
    assert_eq!(
        first.read_typed::<[u8; 4]>(MemoryAddress::new(0x1000), MemoryAlignment::ONE)?,
        [0x11; 4]
    );
    assert_eq!(
        second.read_typed::<[u8; 4]>(MemoryAddress::new(0x1000), MemoryAlignment::ONE)?,
        [0x33; 4]
    );
    Ok(())
}

#[test]
fn missing_function_binding_never_falls_back_to_another_space() -> TestResult {
    // Given: a firmware-programmed owner for which the platform supplied no memory.
    let memory = space([0x11; 4])?;
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(WINDOWS)?)?;
    windows.management_write(
        WINDOWS + WindowControl::OWNER.offset(),
        &9_u32.to_le_bytes(),
    )?;
    let mut dma = Dma::with_sideband(
        RoutedMemory::new(Arc::new(Mutex::new(windows)), vec![memory.clone()])?,
        1,
        |access| access,
    )?;
    let context = dma.channel_context(ChannelId::new(0))?;
    let address = MemoryAddress::new(0x1000);
    let mut bytes = [0xa5; 4];
    // When: that channel validates, reads and writes an address installed in Function zero.
    let checked = context.memory.validate(address, MemoryAlignment::ONE, 4);
    let read = context
        .memory
        .read(address, MemoryAlignment::ONE, &mut bytes);
    let write = context
        .memory
        .write(address, MemoryAlignment::ONE, &[0x33; 4]);
    // Then: all accesses fail without reading or modifying the other Function's bytes.
    for result in [checked, read, write] {
        assert_eq!(
            result,
            Err(RouteError::Window(
                pdma_model_hw_components::windows::MemoryError::Downstream(
                    SpaceError::MissingSpace {
                        identity: MemoryIdentity {
                            function: FunctionId::new(9),
                            attributes: ContextAttributes::DEFAULT
                        },
                        address,
                    }
                )
            ))
        );
    }
    assert_eq!(bytes, [0xa5; 4]);
    assert_eq!(
        memory.read_typed::<[u8; 4]>(address, MemoryAlignment::ONE)?,
        [0x11; 4]
    );
    Ok(())
}

fn space(bytes: [u8; 4]) -> Result<SharedMemory, Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    let address = MemoryAddress::new(0x1000);
    memory.add_regions(&[MemoryRegion::try_new(address, 4, MemoryAlignment::ONE)?])?;
    memory.write_typed(address, MemoryAlignment::ONE, &bytes)?;
    Ok(memory)
}
