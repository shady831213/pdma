use super::*;
use hardware_interface_runtime::AtomicReadOrder;

#[test]
fn unsupported_context_and_control_ports_do_not_emulate_atomics() {
    // Given: an ordinary-only backend with initialized bytes.
    let backing = crate::support::Spaces::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = table(&windows).unwrap();
    let port = table.memory(&windows, &backing);
    let control = ControlChannel::new(0, FunctionId::new(0));
    let window = windows
        .memory(&backing)
        .map_sideband(|_, function| MemoryIdentity {
            function,
            attributes: ContextAttributes::DEFAULT,
        });
    // When: both ports receive both supported-width operations.
    assert_eq!(
        port.atomic32_with_sideband(
            &access(0),
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Exchange(99),
                order: AtomicOrder::Relaxed,
            }
        ),
        Err(AtomicAccessError::Unsupported)
    );
    assert_eq!(
        port.atomic64_with_sideband(
            &access(0),
            ADDRESS,
            AtomicOperation::Load(AtomicReadOrder::Acquire)
        ),
        Err(AtomicAccessError::Unsupported)
    );
    assert_eq!(
        control.memory(&backing).atomic32(
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Exchange(99),
                order: AtomicOrder::Relaxed,
            }
        ),
        Err(AtomicAccessError::Unsupported)
    );
    assert_eq!(
        control
            .memory(&backing)
            .atomic64(ADDRESS, AtomicOperation::Load(AtomicReadOrder::Acquire)),
        Err(AtomicAccessError::Unsupported)
    );
    // Then: no ordinary-write fallback modified the initialized storage.
    assert_eq!(
        window.atomic32_with_sideband(
            &ChannelId::new(0),
            ADDRESS,
            AtomicOperation::Load(AtomicReadOrder::Acquire)
        ),
        Err(AtomicAccessError::Unsupported)
    );
    assert_eq!(
        window.atomic64_with_sideband(
            &ChannelId::new(0),
            ADDRESS,
            AtomicOperation::Load(AtomicReadOrder::Acquire)
        ),
        Err(AtomicAccessError::Unsupported)
    );
    assert_eq!(
        backing
            .0
            .first()
            .unwrap()
            .1
            .read_typed::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [11; 4]
    );
}

#[test]
fn context_and_control_atomics_preserve_downstream_failure_without_modification() {
    // Given: authorized context and PF ports over mapped backing.
    let backing = Backing::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = table(&windows).unwrap();
    let context = table.memory(&windows, &backing);
    let control = ControlChannel::new(0, FunctionId::new(0));
    let address = MemoryAddress::new(0x2000);
    // When: each width targets an unmapped address through both ports.
    let context32 = context.atomic32_with_sideband(
        &access(0),
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(3),
            order: AtomicOrder::Relaxed,
        },
    );
    let context64 = context.atomic64_with_sideband(
        &access(0),
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(3),
            order: AtomicOrder::Relaxed,
        },
    );
    let control32 = control.memory(&backing).atomic32(
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(3),
            order: AtomicOrder::Relaxed,
        },
    );
    let control64 = control.memory(&backing).atomic64(
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(3),
            order: AtomicOrder::Relaxed,
        },
    );
    // Then: all typed error chains retain the address and no backing changes.
    let context_error = AtomicAccessError::Access(context::MemoryError::Downstream(
        MemoryAccessError::Unmapped { address },
    ));
    assert_eq!(context32, Err(context_error));
    assert_eq!(
        context64,
        Err(AtomicAccessError::Access(context::MemoryError::Downstream(
            MemoryAccessError::Unmapped { address }
        )))
    );
    assert_eq!(
        control32,
        Err(AtomicAccessError::Access(
            pdma_model_hw_components::control::MemoryError::Downstream(
                MemoryAccessError::Unmapped { address }
            )
        ))
    );
    assert_eq!(
        control64,
        Err(AtomicAccessError::Access(
            pdma_model_hw_components::control::MemoryError::Downstream(
                MemoryAccessError::Unmapped { address }
            )
        ))
    );
    assert_eq!(backing.values().unwrap(), [10, 11, 12]);
}

#[test]
fn window_atomic_rejects_unowned_channel_before_downstream() {
    // Given: only channel zero exists.
    let backing = Backing::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let port = windows.memory(&backing).map_sideband(|_, function| {
        (
            MemoryIdentity {
                function,
                attributes: ContextAttributes::DEFAULT,
            },
            "denied",
        )
    });
    let channel = ChannelId::new(1);
    // When: both widths target the nonexistent channel.
    assert_eq!(
        port.atomic32_with_sideband(
            &channel,
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Exchange(99),
                order: AtomicOrder::Relaxed,
            }
        ),
        Err(AtomicAccessError::Access(
            pdma_model_hw_components::windows::MemoryError::UnownedChannel { channel }
        ))
    );
    assert_eq!(
        port.atomic64_with_sideband(
            &channel,
            ADDRESS,
            AtomicOperation::Update {
                update: AtomicUpdate::Exchange(99),
                order: AtomicOrder::Relaxed,
            }
        ),
        Err(AtomicAccessError::Access(
            pdma_model_hw_components::windows::MemoryError::UnownedChannel { channel }
        ))
    );
    // Then: the backend sees neither access and every byte stays intact.
    assert!(backing.reports.borrow().is_empty());
    assert_eq!(backing.values().unwrap(), [10, 11, 12]);
}
