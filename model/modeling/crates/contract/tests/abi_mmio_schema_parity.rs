//! Literal parity coverage for the generated PDMA MMIO schema map.

#[path = "abi_mmio/register_literals/mod.rs"]
pub mod expected;

use std::{fmt::Debug, hash::Hash};

use hardware_interface_schema::register_map::RegisterLocation;
use pdma_model_contract::abi::mmio::{MMIO_REGION_SIZE, PdmaMmioMap, PdmaMmioMapRegisterId};

#[test]
fn generated_pdma_map_preserves_all_literal_rows_and_exact_starts() {
    // Given: the independent 66-row v0.9 literal table and generated map ID type.
    assert_generated_id_derives::<PdmaMmioMapRegisterId>();
    assert_eq!(expected::len(), 66);
    assert_eq!(MMIO_REGION_SIZE, 0x1000);
    assert_eq!(PdmaMmioMap::SPEC.size(), 0x1000);

    // When: every literal row is queried through the sole generated placement graph.
    for (id, name, offset, width, access, reset, reset_value, writable_mask) in expected::iter() {
        let actual = PdmaMmioMap::register(id);
        assert_eq!(actual.name(), name);
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
    for offset in [0x001, 0x00c, 0x011, 0x16d, 0x271, 0xfff, 0x1000] {
        assert_eq!(PdmaMmioMap::locate(offset), None);
    }
}

const fn assert_generated_id_derives<T>()
where
    T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
{
}
