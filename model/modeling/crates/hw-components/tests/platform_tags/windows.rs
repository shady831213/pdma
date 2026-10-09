use super::{
    Operation, PlatformMemory, Sideband, Tag,
    backend::{ADDRESS, Error},
    tag,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{
    dma::ChannelId,
    memory::{ContextAttributes, MemoryIdentity},
};
use pdma_model_hw_components::windows::{ChannelWindows, MemoryError};
use std::cell::Cell;

#[test]
fn platform_tags_distinguish_same_function_channels_without_context_table() {
    // Given: two channels owned by the same Function and a platform-defined sideband.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let backend = PlatformMemory::new().unwrap();
    let port = windows
        .memory(&backend)
        .map_sideband(|channel, function| Sideband {
            identity: MemoryIdentity {
                function,
                attributes: ContextAttributes::DEFAULT,
            },
            tag: tag(channel),
        });
    // When: each channel validates, writes and reads the same address.
    for channel in [ChannelId::new(0), ChannelId::new(1)] {
        port.validate_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, 4)
            .unwrap();
        port.write_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &[37; 4])
            .unwrap();
        assert_eq!(
            port.read_typed_with_sideband::<[u8; 4]>(&channel, ADDRESS, MemoryAlignment::ONE)
                .unwrap(),
            [37; 4]
        );
    }
    // Then: all methods deliver distinct opaque tokens while preserving the common owner.
    let reports = backend.reports.borrow();
    let expected: Vec<_> = [
        Tag::Amber([0xe1, 0x37, 0x92]),
        Tag::Indigo("platform-selected"),
    ]
    .into_iter()
    .flat_map(|tag| {
        [Operation::Validate, Operation::Write, Operation::Read].map(|op| (tag.clone(), op))
    })
    .collect();
    assert_eq!(
        reports
            .iter()
            .map(|(sideband, op)| (sideband.tag.clone(), *op))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        reports
            .iter()
            .all(|(sideband, _)| sideband.identity.function.raw() == 0)
    );
}

#[test]
fn unowned_channel_never_calls_platform_mapper_or_backend() {
    // Given: an out-of-range channel and a mapper with observable effects.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let backend = PlatformMemory::new().unwrap();
    let calls = Cell::new(0);
    let port = windows.memory(&backend).map_sideband(|channel, function| {
        calls.set(calls.get() + 1);
        Sideband {
            identity: MemoryIdentity {
                function,
                attributes: ContextAttributes::DEFAULT,
            },
            tag: tag(channel),
        }
    });
    let channel = ChannelId::new(2);
    let mut bytes = [99; 4];
    // When: each memory method requests an unowned channel.
    let results = [
        port.validate_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, 4),
        port.read_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &[37; 4]),
    ];
    // Then: hardware rejection occurs before any platform side effect.
    for result in results {
        assert_eq!(result, Err(MemoryError::UnownedChannel { channel }));
    }
    assert_eq!(calls.get(), 0);
    assert!(backend.reports.borrow().is_empty());
    assert_eq!(bytes, [99; 4]);
}

#[test]
fn mapped_channel_preserves_custom_downstream_errors() {
    // Given: an authorized channel and a backend reporting a typed missing-page error.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let backend = PlatformMemory::new().unwrap();
    backend.fault.set(true);
    let port = windows
        .memory(&backend)
        .map_sideband(|channel, function| Sideband {
            identity: MemoryIdentity {
                function,
                attributes: ContextAttributes::DEFAULT,
            },
            tag: tag(channel),
        });
    let channel = ChannelId::new(1);
    let mut bytes = [99; 4];
    // When: each memory method reaches the failing platform backend.
    let results = [
        port.validate_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, 4),
        port.read_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write_with_sideband(&channel, ADDRESS, MemoryAlignment::ONE, &[37; 4]),
    ];
    // Then: each original downstream error is retained.
    for result in results {
        assert_eq!(
            result,
            Err(MemoryError::Downstream(Error::MissingPage(ADDRESS)))
        );
    }
    assert_eq!(bytes, [99; 4]);
}
