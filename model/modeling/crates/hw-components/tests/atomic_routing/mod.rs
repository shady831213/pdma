use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicUpdate, MemoryAccess, MemoryAccessError,
    MemoryAddress, MemoryAlignment, SharedMemory,
};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, DmaAccess},
    hw_components::context::{ContextEntryRegisters, ContextTableRegisters},
    hw_components::control::ControlRegisters,
    memory::{ContextAttributes, FunctionId, MemoryIdentity},
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::{context, control::ControlChannel, windows::ChannelWindows};

mod backend;
mod rejection;
use backend::{ADDRESS, Backing};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn table(windows: &ChannelWindows) -> TestResult<context::ContextTable> {
    let mut dma = Dma::new(SharedMemory::new(), 1)?;
    let mut table = context::ContextTable::new(4, 1, 0)?;
    let layout = ContextTableRegisters::for_channels(4, 1)?;
    for (index, owner, attributes) in [(0, 0_u32, 0_u32), (1, 0, 1), (2, 1, 0)] {
        let base = layout
            .entries()
            .offset(index)
            .ok_or("missing context entry")?;
        for (offset, value) in [
            (ContextEntryRegisters::OWNER.offset(), owner),
            (ContextEntryRegisters::ATTRIBUTES.offset(), attributes),
            (ContextEntryRegisters::VALID.offset(), 1),
        ] {
            table.management_write((base + offset, &value.to_le_bytes()), (&mut dma, windows))?;
        }
    }
    Ok(table)
}

const fn access(context: u32) -> DmaAccess {
    DmaAccess {
        channel: ChannelId::new(0),
        context: ContextId::new(context),
    }
}

#[test]
fn context_atomic_routes_default_and_pasid_zero_with_opaque_source() {
    // Given: independent Default and PASID-zero backings and a platform-owned tag.
    let backing = Backing::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = table(&windows).unwrap();
    let port = table
        .memory(&windows, &backing)
        .map_sideband(|_, identity| (identity, "opaque-source"));
    // When: the same VA is atomically updated through each context at both widths.
    for context in [0, 1] {
        assert_eq!(
            port.atomic64_with_sideband(
                &access(context),
                ADDRESS,
                AtomicOperation::Update {
                    update: AtomicUpdate::Add(7),
                    order: AtomicOrder::AcquireRelease,
                }
            )
            .unwrap(),
            10 + u64::from(context)
        );
        assert_eq!(
            port.atomic32_with_sideband(
                &access(context),
                ADDRESS,
                AtomicOperation::Update {
                    update: AtomicUpdate::Add(3),
                    order: AtomicOrder::Relaxed,
                }
            )
            .unwrap(),
            17 + context
        );
    }
    // Then: each backing changes independently, with exactly one atomic per request.
    assert_eq!(backing.values().unwrap(), [20, 21, 12]);
    let reports = backing.reports.borrow();
    assert_eq!(reports.len(), 4);
    for (index, (identity, tag)) in reports.iter().enumerate() {
        assert_eq!(identity, &backing.spaces.get(index / 2).unwrap().0);
        assert_eq!(*tag, "opaque-source");
    }
}

#[test]
fn context_atomic_rejections_preserve_all_backing_bytes() {
    // Given: one foreign context, one invalid context, and an unowned channel.
    let backing = Backing::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = table(&windows).unwrap();
    let port = table
        .memory(&windows, &backing)
        .map_sideband(|_, identity| (identity, "denied"));
    let cases = [
        (
            access(2),
            context::MemoryError::OwnerMismatch {
                context: ContextId::new(2),
                owner: FunctionId::new(0),
            },
        ),
        (
            access(3),
            context::MemoryError::InvalidContext {
                context: ContextId::new(3),
            },
        ),
        (
            DmaAccess {
                channel: ChannelId::new(1),
                context: ContextId::new(0),
            },
            context::MemoryError::UnownedChannel {
                channel: ChannelId::new(1),
            },
        ),
    ];
    // When: both widths attempt to modify rejected selectors.
    for (selector, error) in cases {
        assert_eq!(
            port.atomic64_with_sideband(
                &selector,
                ADDRESS,
                AtomicOperation::Update {
                    update: AtomicUpdate::Exchange(99),
                    order: AtomicOrder::Relaxed,
                }
            ),
            Err(AtomicAccessError::Access(error.clone()))
        );
        assert_eq!(
            port.atomic32_with_sideband(
                &selector,
                ADDRESS,
                AtomicOperation::Update {
                    update: AtomicUpdate::Exchange(99),
                    order: AtomicOrder::Relaxed,
                }
            ),
            Err(AtomicAccessError::Access(error))
        );
    }
    // Then: authorization rejects before calling the downstream.
    assert_eq!(backing.values().unwrap(), [10, 11, 12]);
    assert!(backing.reports.borrow().is_empty());
}

#[test]
fn window_atomic_uses_register_owner_and_retains_unmapped_error() {
    // Given: a channel owned by Function one and independently mapped memories.
    let backing = Backing::new().unwrap();
    let mut windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    windows
        .management_write(
            0x10000 + pdma_model_contract::hw_components::windows::WindowControl::OWNER.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    let port = windows.memory(&backing).map_sideband(|_, function| {
        (
            MemoryIdentity {
                function,
                attributes: ContextAttributes::DEFAULT,
            },
            "window",
        )
    });
    // When: an atomic updates the owned space and another targets unmapped storage.
    assert_eq!(
        port.atomic64_with_sideband(
            &ChannelId::new(0),
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Add(8),
                order: AtomicOrder::Relaxed,
            }
        )
        .unwrap(),
        12
    );
    let address = MemoryAddress::new(0x2000);
    let error = port.atomic32_with_sideband(
        &ChannelId::new(0),
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(8),
            order: AtomicOrder::Relaxed,
        },
    );
    // Then: only the owner changes, and backend detail survives the routing wrapper.
    assert_eq!(backing.values().unwrap(), [10, 11, 20]);
    assert_eq!(
        error,
        Err(AtomicAccessError::Access(
            pdma_model_hw_components::windows::MemoryError::Downstream(
                MemoryAccessError::Unmapped { address }
            )
        ))
    );
}

#[test]
fn control_atomic_uses_independent_pf_pasid_configuration() {
    // Given: PF control memory selects PASID zero without a context table.
    let backing = Backing::new().unwrap();
    let mut control = ControlChannel::new(0, FunctionId::new(0));
    control
        .host_write(
            FunctionId::new(0),
            ControlRegisters::ATTRIBUTES.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    let port = control.memory(&backing);
    // When: firmware performs atomic modifications at both widths.
    let before64 = port
        .atomic64(
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Add(5),
                order: AtomicOrder::Release,
            },
        )
        .unwrap();
    let before32 = port
        .atomic32(
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Add(2),
                order: AtomicOrder::Acquire,
            },
        )
        .unwrap();
    // Then: only PF PASID zero changed, and both operations returned old bits.
    assert_eq!((before64, before32), (11, 16));
    assert_eq!(backing.values().unwrap(), [10, 18, 12]);
}
