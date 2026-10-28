//! Hardware identity translation over a Function-tagged downstream memory port.

use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::memory::{MemoryAccessError, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::windows::{ChannelWindows, MemoryError};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const BASE: usize = 0x10_0000;
const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);
const CHANNEL: ChannelId = ChannelId::new(0);

#[test]
fn atomic_forwarding_preserves_backend_failures_and_unsupported() {
    use hw_model_io::memory::{AtomicAccessError, AtomicOperation, AtomicWriteOrder};
    // Given: a channel backed by real memory, plus a backend without atomic support.
    let spaces = Spaces::new().unwrap();
    let windows = ChannelWindows::new(1, 0, u32::try_from(BASE).unwrap()).unwrap();
    let backing = spaces.0.first().unwrap();
    let port = windows.memory(backing).map_sideband(|_, _| ());
    let bad = MemoryAddress::new(0x1001);
    // When: an aligned atomic executes, then misaligned and unsupported requests are issued.
    let changed = port.atomic32_with_sideband(
        &CHANNEL,
        ADDRESS,
        AtomicOperation::Store {
            value: 7,
            order: AtomicWriteOrder::Release,
        },
    );
    let failed = port.atomic64_with_sideband(
        &CHANNEL,
        bad,
        AtomicOperation::Store {
            value: 9,
            order: AtomicWriteOrder::Release,
        },
    );
    let unsupported = windows.memory(&spaces).atomic32_with_sideband(
        &CHANNEL,
        ADDRESS,
        AtomicOperation::Store {
            value: 9,
            order: AtomicWriteOrder::Release,
        },
    );
    // Then: downstream details and capability rejection survive, with no failed-call mutation.
    assert_eq!(changed, Ok(u32::from_le_bytes([1; 4])));
    assert_eq!(
        failed,
        Err(AtomicAccessError::Access(MemoryError::Downstream(
            MemoryAccessError::Misaligned {
                address: bad,
                alignment: MemoryAlignment::try_new(8).unwrap()
            }
        )))
    );
    assert_eq!(unsupported, Err(AtomicAccessError::Unsupported));
    assert_eq!(
        backing
            .read_buffer::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        7_u64.to_le_bytes()
    );
}

struct Spaces([SharedMemory; 2]);

impl Spaces {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let spaces = [SharedMemory::new(), SharedMemory::new()];
        for (index, memory) in spaces.iter().enumerate() {
            let length = 8 * (u64::try_from(index)? + 1);
            memory.add_regions(&[MemoryRegion::try_new(
                ADDRESS,
                length,
                MemoryAlignment::ONE,
            )?])?;
            memory.write(
                ADDRESS,
                MemoryAlignment::ONE,
                &[u8::try_from(index)? + 1; 4],
            )?;
        }
        Ok(Self(spaces))
    }

    fn get(
        &self,
        function: FunctionId,
        address: MemoryAddress,
    ) -> Result<&SharedMemory, MemoryAccessError> {
        usize::try_from(function.raw())
            .ok()
            .and_then(|index| self.0.get(index))
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

impl MemoryAccess<FunctionId> for Spaces {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .write(address, alignment, bytes)
    }
}

#[test]
fn channel_access_uses_owner_for_validation_raw_and_typed_io() -> TestResult {
    // Given: identical addresses in two Function spaces with different extents and bytes.
    let spaces = Spaces::new()?;
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE)?)?;
    windows.management_write(BASE + WindowControl::OWNER.offset(), &1_u32.to_le_bytes())?;
    let downstream: &dyn MemoryAccess<FunctionId, Error = MemoryAccessError> = &spaces;
    let port = windows.memory(downstream);
    // When: channel zero accesses the second Function through the hardware view.
    port.validate_with_sideband(&CHANNEL, ADDRESS, MemoryAlignment::ONE, 16)?;
    let before =
        port.read_buffer_with_sideband::<[u8; 4]>(&CHANNEL, ADDRESS, MemoryAlignment::ONE)?;
    port.write_buffer_with_sideband(&CHANNEL, ADDRESS, MemoryAlignment::ONE, &[7; 4])?;
    let mut after = [0; 4];
    port.read_with_sideband(&CHANNEL, ADDRESS, MemoryAlignment::ONE, &mut after)?;
    // Then: only the configured Function's bytes change, even through an erased downstream.
    assert_eq!(before, [2; 4]);
    assert_eq!(after, [7; 4]);
    assert_eq!(
        spaces
            .get(FunctionId::new(0), ADDRESS)?
            .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)?,
        [1; 4]
    );
    Ok(())
}

#[test]
fn closing_host_access_retains_owner_for_old_memory_requests() -> TestResult {
    // Given: an open channel owned by Function one.
    let spaces = Spaces::new()?;
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE)?)?;
    windows.management_write(BASE + WindowControl::OWNER.offset(), &1_u32.to_le_bytes())?;
    windows.management_write(BASE, &1_u32.to_le_bytes())?;
    // When: firmware closes Host access before draining old work.
    windows.management_write(BASE, &0_u32.to_le_bytes())?;
    let bytes = windows
        .memory(&spaces)
        .read_buffer_with_sideband::<[u8; 4]>(&CHANNEL, ADDRESS, MemoryAlignment::ONE)?;
    // Then: memory still uses the retained owner, rather than rejecting or falling back to PF.
    assert_eq!(bytes, [2; 4]);
    Ok(())
}

#[test]
fn later_requests_observe_reassigned_owner_without_cached_identity() -> TestResult {
    // Given: a closed idle channel initially owned by Function zero.
    let spaces = Spaces::new()?;
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE)?)?;
    assert_eq!(
        windows
            .memory(&spaces)
            .read_buffer_with_sideband::<[u8; 4]>(&CHANNEL, ADDRESS, MemoryAlignment::ONE)?,
        [1; 4]
    );
    // When: firmware assigns the already-idle channel to Function one.
    windows.management_write(BASE + WindowControl::OWNER.offset(), &1_u32.to_le_bytes())?;
    let bytes = windows
        .memory(&spaces)
        .read_buffer_with_sideband::<[u8; 4]>(&CHANNEL, ADDRESS, MemoryAlignment::ONE)?;
    // Then: subsequent requests use the new register value.
    assert_eq!(bytes, [2; 4]);
    Ok(())
}

#[test]
fn atomic_requests_observe_reassigned_owner_without_cached_identity() -> TestResult {
    use hw_model_io::memory::{AtomicOperation, AtomicWriteOrder};
    // Given: a closed channel and a platform mapper observing its authorized owner.
    let spaces = Spaces::new()?;
    let backing = spaces.0.first().ok_or("missing backing")?;
    let owners = std::cell::RefCell::new(Vec::new());
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE)?)?;
    let map = |_, owner| owners.borrow_mut().push(owner);
    let first = windows
        .memory(backing)
        .map_sideband(map)
        .atomic32_with_sideband(
            &CHANNEL,
            ADDRESS,
            AtomicOperation::Store {
                value: 7,
                order: AtomicWriteOrder::Release,
            },
        )?;
    // When: firmware reassigns the channel before a subsequent atomic access.
    windows.management_write(BASE + WindowControl::OWNER.offset(), &1_u32.to_le_bytes())?;
    let second = windows
        .memory(backing)
        .map_sideband(map)
        .atomic64_with_sideband(
            &CHANNEL,
            ADDRESS,
            AtomicOperation::Store {
                value: 9,
                order: AtomicWriteOrder::Release,
            },
        )?;
    // Then: each access maps the current register owner and performs its real atomic effect.
    assert_eq!(first, u32::from_le_bytes([1; 4]));
    assert_eq!(second, 7);
    assert_eq!(
        *owners.borrow(),
        vec![FunctionId::new(0), FunctionId::new(1)]
    );
    assert_eq!(
        backing.read_buffer::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)?,
        9_u64.to_le_bytes()
    );
    Ok(())
}

#[test]
fn invalid_channel_and_downstream_errors_preserve_buffers() -> TestResult {
    // Given: a mapped Function-zero space and a channel view over it.
    let spaces = Spaces::new()?;
    let windows = ChannelWindows::new(1, 0, u32::try_from(BASE)?)?;
    let port = windows.memory(&spaces);
    let invalid = ChannelId::new(1);
    let mut bytes = [0xa5; 4];
    let misaligned = MemoryAddress::new(0x1001);
    let alignment = MemoryAlignment::try_new(4)?;
    // When: invalid-channel operations and a valid-channel misaligned write are issued.
    let read = port.read_with_sideband(&invalid, ADDRESS, MemoryAlignment::ONE, &mut bytes);
    let write = port.write_with_sideband(&invalid, ADDRESS, MemoryAlignment::ONE, &[9; 4]);
    let validate = port.validate_with_sideband(&invalid, ADDRESS, MemoryAlignment::ONE, 4);
    let downstream_error = port.write_with_sideband(&CHANNEL, misaligned, alignment, &[9; 4]);
    // Then: errors retain their addresses, no read buffer or backing bytes are changed.
    assert_eq!(read, Err(MemoryError::UnownedChannel { channel: invalid }));
    assert_eq!(write, Err(MemoryError::UnownedChannel { channel: invalid }));
    assert_eq!(
        validate,
        Err(MemoryError::UnownedChannel { channel: invalid })
    );
    assert_eq!(
        downstream_error,
        Err(MemoryError::Downstream(MemoryAccessError::Misaligned {
            address: misaligned,
            alignment
        }))
    );
    assert_eq!(bytes, [0xa5; 4]);
    assert_eq!(
        spaces
            .get(FunctionId::new(0), ADDRESS)?
            .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)?,
        [1; 4]
    );
    Ok(())
}
