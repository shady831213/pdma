//! Facade macro re-export smoke test.

use hw_model_schema::{bits_schema, record_schema, register_map, register_schema};

bits_schema! { pub bits SurfaceBits(u8) { enable: 0..1, } }

record_schema! {
    pub record SurfaceRecord {
        alignment: 4,
        endian: Little,
        fields: {
            value: u8,
        },
    }
}

register_schema! { pub register SurfaceControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: SurfaceBits,
} }

register_map! { pub map SurfaceChannelMap {
    alignment: 4, extent: 8,
    placements: [control: SurfaceControl],
} }

register_map! { pub map SurfaceComposableMap {
    alignment: 4,
    extent: 32,
    placements: [
        control: SurfaceControl,
        channel: SurfaceChannelMap[2] { offset: 16 },
    ],
} }

register_schema! {
    pub register SurfaceValue {
        width: 4, endian: Little, access: ReadWrite,
        reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
    }
}

register_map! {
    pub map SurfaceMap {
        alignment: 4,
        extent: 8,
        placements: [value: SurfaceValue],
    }
}

#[test]
fn facade_reexports_macros_when_surface_is_declared() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(SurfaceRecord::SPEC.size(), 4);
    assert_eq!(SurfaceValue::SPEC.width(), 4);
    assert_eq!(SurfaceMap::EXTENT, 8);
    let resolved = SurfaceComposableMap::resolve()?;
    let paths = resolved
        .registers()
        .iter()
        .map(hw_model_schema::ResolvedRegister::path)
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
        .channel_mut(1)?
        .control_mut()?
        .set_fields(SurfaceBitsValues { enable: true })?;
    assert_eq!(bytes[24], 1);
    Ok(())
}
