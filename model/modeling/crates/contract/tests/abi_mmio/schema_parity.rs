//! Literal parity coverage for the generated PDMA MMIO schema map.

use super::register_literals as expected;

use std::{fmt::Debug, hash::Hash};

use hardware_interface_schema::register_map::RegisterLocation;
use pdma_model_contract::abi::mmio::{
    MMIO_REGION_SIZE, PdmaMmioMap, PdmaMmioMapRegisterId, register_at,
};

#[test]
fn generated_pdma_map_preserves_all_literal_rows_and_exact_starts() {
    // Given: the independent 66-row v0.9 literal table and generated map ID type.
    assert_generated_id_derives::<PdmaMmioMapRegisterId>();
    assert_eq!(expected::len(), 66);
    assert_eq!(MMIO_REGION_SIZE, 0x1000);
    assert_eq!(PdmaMmioMap::EXTENT, 0x1000);
    let mut identities = std::collections::BTreeSet::new();

    // When: every literal row is queried through the sole generated placement graph.
    for (id, name, offset, width, access, reset, reset_value, writable_mask) in expected::iter() {
        let actual =
            register_at(usize::try_from(offset).unwrap()).expect("exact leaf address resolves");
        assert!(identities.insert((actual.id(), actual.offset())));
        assert_eq!(actual.name(), name.rsplit('.').next().unwrap());
        assert_eq!(actual.offset(), usize::try_from(offset).unwrap());
        assert_eq!(actual.width(), usize::try_from(width).unwrap());
        assert_eq!(actual.access(), access);
        assert_eq!(actual.reset_kind(), reset);
        assert_eq!(actual.reset_value(), reset_value);
        assert_eq!(actual.writable_mask(), writable_mask);
        assert!(matches!(
            PdmaMmioMap::locate(usize::try_from(offset).unwrap()),
            Some(RegisterLocation::Fixed { register, offset: found })
                if register == actual && register.id() == id && found == usize::try_from(offset).unwrap()
        ));
    }

    // Then: interior, reserved, region-end, and out-of-region offsets never locate.
    assert_eq!(identities.len(), 66);
    for offset in [0x001, 0x00c, 0x011, 0x16d, 0x271, 0xfff, 0x1000] {
        assert_eq!(PdmaMmioMap::locate(offset), None);
    }
}

const fn assert_generated_id_derives<T>()
where
    T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
{
}

#[test]
fn literal_register_inventory_is_exact() {
    assert_eq!(MMIO_REGION_SIZE, 0x1000);
    let resolved = PdmaMmioMap::resolve().expect("canonical MMIO map must resolve");
    let registers = resolved.registers();
    assert_eq!(registers.len(), expected::len());
    for (actual, expected) in registers.iter().zip(expected::iter()) {
        let definition = actual.definition();
        assert_eq!(
            (
                actual.id(),
                actual
                    .path()
                    .strip_prefix("PdmaMmioMap.")
                    .unwrap()
                    .to_uppercase()
                    .as_str(),
                u32::try_from(actual.offset()).expect("literal offset fits u32"),
                u128::try_from(definition.width()).expect("literal width fits u128"),
                definition.access(),
                definition.reset_kind(),
                definition.reset_value(),
                definition.writable_mask()
            ),
            expected
        );
        assert_eq!(actual.offset() % definition.width(), 0);
        assert!(actual.offset() + actual.definition().width() <= PdmaMmioMap::EXTENT);
    }
    println!("literal MMIO registers checked: {}", expected::len());
}
