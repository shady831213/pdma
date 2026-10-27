use super::*;
use crate::harness::runtime::memory::SpaceError;
use hardware_interface_memory::MemoryAccessError;

struct Ordinary(SharedMemory);

impl MemoryAccess for Ordinary {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.0.validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.0.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.0.write(address, alignment, bytes)
    }
}

#[test]
fn authorized_atomic_requests_preserve_unsupported_through_managed_path() {
    // Given: an authorized owner with a backend implementing only ordinary access.
    let backing = space(31);
    let windows = ChannelWindows::new(1, 0, u32::try_from(BASE).unwrap()).unwrap();
    let memory = RoutedMemory::new(
        Arc::new(Mutex::new(windows)),
        vec![Ordinary(backing.clone())],
    )
    .unwrap();
    // When: both word widths request atomic stores.
    let result32 = memory.atomic32_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    let result64 = memory.atomic64_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    // Then: Unsupported stays distinct from access errors and no ordinary-write emulation occurs.
    assert_eq!(result32, Err(AtomicAccessError::Unsupported));
    assert_eq!(result64, Err(AtomicAccessError::Unsupported));
    assert_eq!(
        backing
            .read_typed::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        31_u64.to_le_bytes()
    );
}

#[test]
fn atomic_requests_reject_missing_space_without_fallback_or_modification() {
    // Given: the current owner has no platform backing, while Function zero does.
    let backing = space(41);
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE).unwrap()).unwrap();
    windows
        .management_write(BASE + WindowControl::OWNER.offset(), &9_u32.to_le_bytes())
        .unwrap();
    let memory = RoutedMemory::new(Arc::new(Mutex::new(windows)), vec![backing.clone()]).unwrap();
    // When: both widths attempt to store at the other Function's mapped address.
    let result32 = memory.atomic32_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    let result64 = memory.atomic64_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    // Then: MissingSpace retains the selected owner and address; foreign memory remains intact.
    let error = || {
        AtomicAccessError::Access(RouteError::<MemoryAccessError>::Window(
            pdma_model_hw_components::windows::MemoryError::Downstream(SpaceError::MissingSpace {
                identity: MemoryIdentity {
                    function: FunctionId::new(9),
                    attributes: ContextAttributes::DEFAULT,
                },
                address: ADDRESS,
            }),
        ))
    };
    assert_eq!(result32, Err(error()));
    assert_eq!(result64, Err(error()));
    assert_eq!(
        backing
            .read_typed::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        41_u64.to_le_bytes()
    );
}
