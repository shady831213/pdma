//! Facade macro re-export smoke test.

use hardware_interface_schema::{record_schema, register_map, register_schema};

record_schema! {
    pub record SurfaceRecord {
        size: 4,
        alignment: 4,
        fields: {
            value: u32 { id: 1, offset: 0, endian: Little },
        }
    }
}

register_schema! {
    pub registers SurfaceRegisters {
        value: { id: 1, name: "value", offset: 0, width: 4, access: ReadWrite,
            reset: Constant(0), writable_mask: 0xffff_ffff },
    }
}

register_map! {
    pub map SurfaceMap {
        size: 8,
        fixed: {
            value: { id: 1, name: "value", offset: 0, width: 4, access: ReadWrite,
                reset: Constant(0), writable_mask: 0xffff_ffff },
        },
        banks: {}
    }
}

#[test]
fn facade_reexports_macros_when_surface_is_declared() {
    assert_eq!(SurfaceRecord::SPEC.size(), 4);
    assert_eq!(SurfaceRegisters::SPEC.len(), 1);
    assert_eq!(SurfaceMap::SPEC.size(), 8);
}
