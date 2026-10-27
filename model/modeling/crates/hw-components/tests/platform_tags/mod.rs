use hardware_interface_memory::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, DmaAccess},
    hw_components::context::{
        ContextAttributeRegisters, ContextEntryRegisters, ContextTableRegisters,
    },
    memory::{ContextAttributes, ContextAttributesAddressSpace, FunctionId, MemoryIdentity},
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::{context::ContextTable, windows::ChannelWindows};

mod authorization;
mod backend;
mod windows;
use backend::{ADDRESS, Operation, PlatformMemory, Sideband, Tag};

fn configured_table(windows: &ChannelWindows) -> Result<ContextTable, Box<dyn std::error::Error>> {
    let mut dma = Dma::new(SharedMemory::new(), 2)?;
    let mut table = ContextTable::new(3, 2, 0)?;
    let layout = ContextTableRegisters::for_channels(3, 2)?;
    for (index, owner) in [(0, 0_u32), (1, 1)] {
        let base = layout
            .entries()
            .offset(index)
            .ok_or("missing context entry")?;
        for (offset, value) in [
            (ContextEntryRegisters::OWNER.offset(), owner),
            (
                ContextEntryRegisters::ATTRIBUTES.offset()
                    + ContextAttributeRegisters::ADDRESS_SPACE.offset(),
                1,
            ),
            (
                ContextEntryRegisters::ATTRIBUTES.offset()
                    + ContextAttributeRegisters::PASID.offset(),
                19,
            ),
            (ContextEntryRegisters::VALID.offset(), 1),
        ] {
            table.management_write((base + offset, &value.to_le_bytes()), (&mut dma, windows))?;
        }
    }
    Ok(table)
}

const fn identity() -> MemoryIdentity {
    MemoryIdentity {
        function: FunctionId::new(0),
        attributes: ContextAttributes {
            address_space: ContextAttributesAddressSpace::Pasid,
            pasid: 19,
        },
    }
}

const fn sideband(access: DmaAccess, identity: MemoryIdentity) -> Sideband {
    Sideband {
        identity,
        tag: tag(access.channel),
    }
}

const fn tag(channel: ChannelId) -> Tag {
    match channel.index() {
        0 => Tag::Amber([0xe1, 0x37, 0x92]),
        _ => Tag::Indigo("platform-selected"),
    }
}

#[test]
fn platform_reports_distinguish_channels_sharing_function_and_pasid() {
    // Given: two channels with the same Function, context and PASID at the same address.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let backend = PlatformMemory::new().unwrap();
    let port = table.memory(&windows, &backend).map_sideband(sideband);
    let channels = [0, 1];
    let payload = [42; 4];
    // When: both channels validate, write and read through the routing boundary.
    for channel in channels {
        let access = DmaAccess {
            channel: ChannelId::new(channel),
            context: ContextId::new(0),
        };
        port.validate_with_sideband(
            &access,
            ADDRESS,
            MemoryAlignment::ONE,
            u64::try_from(payload.len()).unwrap(),
        )
        .unwrap();
        port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &payload)
            .unwrap();
        assert_eq!(
            port.read_typed_with_sideband::<[u8; 4]>(&access, ADDRESS, MemoryAlignment::ONE)
                .unwrap(),
            payload
        );
    }
    // Then: the backend reports distinct opaque platform tags with unchanged address identity.
    let expected: Vec<_> = channels
        .into_iter()
        .flat_map(|channel| {
            let sideband = sideband(
                DmaAccess {
                    channel: ChannelId::new(channel),
                    context: ContextId::new(0),
                },
                identity(),
            );
            [Operation::Validate, Operation::Write, Operation::Read]
                .map(|op| (sideband.clone(), op))
        })
        .collect();
    assert_eq!(*backend.reports.borrow(), expected);
}
