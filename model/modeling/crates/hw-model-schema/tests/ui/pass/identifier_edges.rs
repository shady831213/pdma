//! Valid Rust identifiers remain usable at generated schema boundaries.

use hw_model_schema::{Decode, Encode, bits_schema, record_schema, register_map, register_schema};

bits_schema! { pub bits EdgeBits(u8) {
    __: 0..1, ___: 1..2, _1: 2..4, r#_2: 4..6, self_: 6..7,
} }
register_schema! {
    pub register EdgeRegister {
        width: 1, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 15,
        fields: { __: 0..1, ___: 1..2, _1: 2..4 },
    }
}
register_map! {
    pub map EdgeMap {
        alignment: 1, placements: [pub __: EdgeRegister, pub _1: EdgeRegister],
        access: [___.value => __, _2.value => _1],
    }
}
register_map! {
    pub map EdgeInstance(_1: usize) {
        alignment: 1, placements: [pub __: EdgeMap[_1]],
    }
}
record_schema! {
    pub record EdgeRecord {
        endian: Little,
        fields: {
            __: u8,
            kind: enum(u8) { variants: { Value = 1 } },
            _1: tagged_union {
                tag: self.kind, cases: { Value { __: u8, _1: u8 } },
            },
        },
    }
}

fn underscore_and_numeric_prefix_bits_when_encoded_round_trip()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: distinct underscore-only and numeric-prefix fields.
    let value = EdgeBitsValues {
        __: true,
        ___: false,
        _1: 3,
        r#_2: 2,
        self_: true,
    };
    // When: the generated semantic codec encodes and decodes these fields.
    let raw = value.encode()?;
    let decoded = raw.decode()?;
    // Then: declared slots retain values and field IDs stay distinct.
    assert_eq!(raw.value(), 109);
    assert_eq!(decoded, value);
    assert_ne!(EdgeBitsFieldId::__, EdgeBitsFieldId::___);
    assert_ne!(EdgeBitsFieldId::__, EdgeBitsFieldId::_1);
    assert_ne!(EdgeBitsFieldId::_2, EdgeBitsFieldId::self_);
    Ok(())
}

fn underscore_and_numeric_prefix_placements_when_accessed_keep_offsets()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two byte-sized placements and a repeated constructor-bound map.
    let mut bytes = [0; 2];
    let layout = EdgeInstance::new(2)?;
    // When: generated placement and register setters update independent slots.
    EdgeMapViewMut::new(&mut bytes)?.___mut()?.set___(true)?;
    EdgeMapViewMut::new(&mut bytes)?._1_mut()?.set__1(2)?;
    // Then: reads, bytes and constructor-bound metadata agree.
    let view = EdgeMapView::new(&bytes)?;
    assert!(view.__()?.__()?);
    assert_eq!(view._1()?._1()?, 2);
    assert_eq!(view._2()?.value()?._1()?, 2);
    assert!(view.___()?.value()?.__()?);
    assert_eq!(bytes, [1, 8]);
    assert_eq!(layout.extent(), 4);
    assert_eq!(layout.resolve()?.registers().len(), 4);
    Ok(())
}

fn numeric_prefix_union_when_encoded_uses_consistent_helper_names()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a numeric-prefix union field containing underscore-named members.
    let value = EdgeRecord::new(
        7,
        EdgeRecordKind::Value,
        EdgeRecord1Values::Value(EdgeRecord1ValueValues { __: 11, _1: 13 }),
    );
    // When: the record and its definition graph describe the same union type.
    let bytes = value.to_bytes()?;
    let decoded = EdgeRecord::from_bytes(&bytes)?;
    // Then: each payload member retains its exact byte position.
    assert_eq!(bytes, [7, 1, 11, 13]);
    assert_eq!(decoded, value);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    underscore_and_numeric_prefix_bits_when_encoded_round_trip()?;
    underscore_and_numeric_prefix_placements_when_accessed_keep_offsets()?;
    numeric_prefix_union_when_encoded_uses_consistent_helper_names()?;
    println!("identifier edge codecs and accessors passed");
    Ok(())
}
