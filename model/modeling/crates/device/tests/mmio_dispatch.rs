//! Device MMIO dispatcher integration coverage.

use pdma_model_contract::abi::mmio::{
    MmioAccess, MmioAccessError, MmioReset, MmioResult, PdmaMmioMap,
};
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};
use pdma_model_device::AsyncDevice;
use std::collections::BTreeSet;

#[path = "mmio_dispatch/malformed.rs"]
mod malformed;
#[path = "mmio_dispatch/support.rs"]
/// Shared observability helpers for malformed-access cases.
pub mod support;

#[tokio::test]
async fn canonical_mmio_map_drives_all_runtime_access_and_reset_behavior() {
    // Given: a fresh device and the fully resolved canonical register map.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let scalar_bytes = |width: usize, value: u64| match width {
        4 => u32::try_from(value)
            .expect("32-bit register metadata value should fit")
            .to_le_bytes()
            .to_vec(),
        8 => value.to_le_bytes().to_vec(),
        _ => panic!("unexpected MMIO register width: {width}"),
    };
    let resolved = PdmaMmioMap::resolve().expect("canonical MMIO map should resolve");
    let mut visited = BTreeSet::new();

    // When/Then: each resolved placement drives its declared raw runtime behavior.
    for register in resolved.registers() {
        let offset = u32::try_from(register.offset()).expect("MMIO offset should fit u32");
        let definition = register.definition();
        let width = definition.width();
        let transaction_width = u128::try_from(width).expect("MMIO width should fit u128");
        assert!(visited.insert(offset), "duplicate placement at {offset:#x}");

        match definition.access() {
            MmioAccess::ReadOnly => {
                assert_eq!(
                    handle.read_mmio(offset, transaction_width).await,
                    Ok(scalar_bytes(width, definition.reset_value())),
                    "construction value for {}",
                    register.name(),
                );
                assert_eq!(
                    handle.write_mmio(offset, &scalar_bytes(width, 0)).await,
                    Err(MmioAccessError::WriteToReadOnly { offset }),
                    "read-only write for {}",
                    register.name(),
                );
            }
            MmioAccess::ReadWrite => {
                assert_eq!(
                    handle.read_mmio(offset, transaction_width).await,
                    Ok(scalar_bytes(width, definition.reset_value())),
                    "construction value for {}",
                    register.name(),
                );
                let writable = scalar_bytes(width, definition.writable_mask());
                assert_eq!(
                    handle.write_mmio(offset, &writable).await,
                    Ok(()),
                    "writable mask for {}",
                    register.name(),
                );
                assert_eq!(
                    handle.read_mmio(offset, transaction_width).await,
                    Ok(writable),
                    "staged writable mask for {}",
                    register.name(),
                );
            }
            MmioAccess::WriteOnly => {
                assert_eq!(
                    handle.read_mmio(offset, transaction_width).await,
                    Err(MmioAccessError::ReadFromWriteOnly { offset }),
                    "write-only read for {}",
                    register.name(),
                );
            }
        }
        println!(
            "placement={} offset={offset:#05x} access={:?} writable_mask={:#x} construction_and_access=pass",
            register.name(),
            definition.access(),
            definition.writable_mask(),
        );
    }

    // When: typed reset advances the epoch and restores the declared readable state.
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await
        .expect("typed reset should succeed");

    // Then: every readable register resets and the typed receipt projects the new epoch.
    for register in resolved.registers() {
        let definition = register.definition();
        if definition.access() == MmioAccess::WriteOnly {
            continue;
        }
        let offset = u32::try_from(register.offset()).expect("MMIO offset should fit u32");
        let expected = match definition.reset_kind() {
            MmioReset::Constant | MmioReset::Zero | MmioReset::PostReset => {
                definition.reset_value()
            }
        };
        let transaction_width =
            u128::try_from(definition.width()).expect("MMIO width should fit u128");
        assert_eq!(
            handle.read_mmio(offset, transaction_width).await,
            Ok(scalar_bytes(definition.width(), expected)),
            "post-reset value for {}",
            register.name(),
        );
    }
    assert_eq!(visited.len(), 66);
    assert_eq!(epoch, DeviceEpoch::new(1));
    println!("placements=66;unique=66;post_reset_epoch={}", epoch.raw());
}

#[tokio::test]
async fn result_and_reset_banks_round_trip_when_commands_are_not_implemented() {
    // Given: a newly constructed device register bank.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();

    // When: representative command writes route into the non-semantic family scaffolds.
    handle
        .write_mmio(0x020, &1_u32.to_le_bytes())
        .await
        .expect("global command scaffold should accept the write");
    handle
        .write_mmio(0x168, &1_u32.to_le_bytes())
        .await
        .expect("queue command scaffold should accept the write");
    handle
        .write_mmio(0x220, &1_u32.to_le_bytes())
        .await
        .expect("IRQ command scaffold should accept the write");
    let magic = handle.read_mmio(0x000, 4).await;
    let version = handle.read_mmio(0x004, 4).await;
    let global_result = handle.read_mmio(0x024, 4).await;
    let queue_result = handle.read_mmio(0x16c, 4).await;
    let irq_result = handle.read_mmio(0x224, 4).await;

    // Then: constants and construction reset values match the canonical encodings.
    assert_eq!(magic, Ok(0x414d_4450_u32.to_le_bytes().to_vec()));
    assert_eq!(version, Ok(1_u32.to_le_bytes().to_vec()));
    let unsupported_version = MmioResult::UnsupportedVersion.code().to_le_bytes().to_vec();
    assert_eq!(global_result, Ok(unsupported_version));
    assert_eq!(
        queue_result,
        Ok(MmioResult::InvalidRingDepth.code().to_le_bytes().to_vec())
    );
    assert_eq!(
        irq_result,
        Ok(MmioResult::InvalidIrqSource.code().to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x028, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x188, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x240, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
}
