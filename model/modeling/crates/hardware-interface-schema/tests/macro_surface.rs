//! Facade macro re-export smoke test.

use hardware_interface_schema::{
    bits_schema, field_schema, record_schema, register_map, register_schema,
};

field_schema! { pub field SurfaceByte { scalar: u8, endian: Little } }
bits_schema! { pub bits SurfaceBits { enable: 0..1, } }

record_schema! {
    pub record SurfaceRecord {
        size: 4,
        alignment: 4,
        fields: {
            value: SurfaceByte { id: 1, offset: 0 },
        },
        records: {},
    }
}

register_schema! { pub register SurfaceControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: SurfaceBits,
} }

register_map! { pub map SurfaceComposableMap {
    size: 32,
    fixed: { control: SurfaceControl { id: 1, name: "control", offset: 0 }, },
    banks: { channel: { count: 2, base: 16, stride: 8, members: {
        control: SurfaceControl { id: 2, name: "control", offset: 0 },
    } }, },
} }

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
fn facade_reexports_macros_when_surface_is_declared() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(SurfaceRecord::SPEC.size(), 4);
    assert_eq!(SurfaceRegisters::SPEC.len(), 1);
    assert_eq!(SurfaceMap::SPEC.size(), 8);
    assert_eq!(SurfaceByte::DEFINITION.width(), 1);
    let resolved = SurfaceComposableMap::resolve()?;
    let paths = resolved
        .registers()
        .iter()
        .map(hardware_interface_schema::ResolvedRegister::path)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "SurfaceComposableMap.control",
            "SurfaceComposableMap.channel[0].control",
            "SurfaceComposableMap.channel[1].control",
        ]
    );
    let mut bytes = [0_u8; 32];
    SurfaceComposableMapViewMut::new(&mut bytes)?
        .channel_control_mut(1)?
        .set_fields(SurfaceBitsValues { enable: true })?;
    assert_eq!(bytes[24], 1);
    Ok(())
}
