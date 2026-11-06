use super::*;
use hw_model_schema::record_schema;

record_schema! {
    pub record Narrow { alignment: 8, endian: Little, fields: { prefix: u8, value: u32 } }
}
record_schema! {
    pub record Wide { alignment: 8, endian: Little, fields: { prefix: u64, value: u32 } }
}
record_schema! {
    pub record Network { alignment: 8, endian: Big, fields: { value: u32 } }
}

#[test]
fn schema_field_changes_propagate_to_offsets_and_accessors() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Narrow>();
    definitions.add::<Wide>();
    let text = render(definitions).unwrap();
    assert!(text.contains("NARROW_VALUE_OFFSET = 4;"));
    assert!(text.contains("WIDE_VALUE_OFFSET = 8;"));
    assert!(
        text.contains("Narrow_get_value[0 +: 8] = record_value[(NARROW_VALUE_OFFSET+0)*8 +: 8]")
    );
    assert!(text.contains("Wide_get_value[0 +: 8] = record_value[(WIDE_VALUE_OFFSET+0)*8 +: 8]"));
}

#[test]
fn scalar_endianness_is_explicit() {
    let mut definitions = SchemaSet::new();
    definitions.add::<Network>();
    let text = render(definitions).unwrap();
    assert!(
        text.contains("Network_get_value[24 +: 8] = record_value[(NETWORK_VALUE_OFFSET+0)*8 +: 8]")
    );
    assert!(text.contains("record_value[(NETWORK_VALUE_OFFSET+0)*8 +: 8] = value[24 +: 8]"));
}
