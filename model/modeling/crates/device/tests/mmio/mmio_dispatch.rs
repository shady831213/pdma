use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use hardware_interface_schema::register::Access;
use pdma_model_contract::abi::registers::{
    GlobalRegisters, PdmaRegisterMap, RegisterLocation, locate,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::AsyncDevice;
use std::collections::BTreeSet;
use std::sync::Arc;

mod malformed;

#[tokio::test]
async fn canonical_register_map_drives_readable_reset_values() {
    // Given: a fresh device and generated direct hardware register placements.
    let (device, _worker) =
        AsyncDevice::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let endpoint = device.handle().register_endpoint();
    let resolved = PdmaRegisterMap::resolve().expect("register map");
    let mut visited = BTreeSet::new();

    // When: each readable register is read before and after direct whole-device reset.
    for epoch in [0_u64, 1] {
        if epoch == 1 {
            endpoint
                .write(
                    u32::try_from(GlobalRegisters::RESET.offset()).expect("offset"),
                    &1_u32.to_le_bytes(),
                )
                .await
                .expect("reset");
        }
        for register in resolved.registers() {
            let offset = u32::try_from(register.offset()).expect("offset");
            let definition = register.definition();
            if definition.access() == Access::WriteOnly {
                continue;
            }
            let mut bytes = vec![0; definition.width()];
            if matches!(locate(offset), Some(RegisterLocation::Irq { .. })) {
                assert!(endpoint.read(offset, &mut bytes).await.is_err());
                continue;
            }
            endpoint
                .read(offset, &mut bytes)
                .await
                .expect("read register");
            let expected = if register.offset() == GlobalRegisters::EPOCH.offset() {
                epoch
            } else {
                definition.reset_value()
            };
            let expected_bytes = expected.to_le_bytes();

            // Then: each value matches the metadata, except the live advancing epoch.
            assert_eq!(
                bytes,
                expected_bytes
                    .get(..definition.width())
                    .expect("register width"),
                "{}",
                register.name()
            );
            visited.insert(offset);
        }
    }
    assert_eq!(
        visited.len(),
        resolved
            .registers()
            .iter()
            .filter(
                |register| register.definition().access() != Access::WriteOnly
                    && !matches!(
                        locate(u32::try_from(register.offset()).expect("offset")),
                        Some(RegisterLocation::Irq { .. })
                    )
            )
            .count()
    );
}
