use super::*;
use hw_model_schema::{ast::Definition, bits_schema, register_schema};
bits_schema! {
    pub bits Flags(u32) {
        mode: 0..=1 => enum { variants: { Idle = 0, Active = 2 } },
        status: 8..=11,
    }
}
register_schema! {
    pub register Shared {
        width: 4, endian: Little, access: ReadWrite,
        reset: Constant(2), writable_mask: 3, fields: Flags,
    }
}
#[test]
fn named_and_direct_fields_preserve_register_encoding_and_enum_metadata() {
    let mut named = SchemaSet::new();
    named.add::<Shared>();
    let mut direct = SchemaSet::new();
    direct.add_definition(Definition::register::<Shared, _>(Shared::SPEC));
    let texts: Vec<_> = [named, direct]
        .into_iter()
        .map(|source| render(source).unwrap())
        .collect();
    for text in &texts {
        assert!(text.contains("uvm_reg_field mode_field;"));
        assert!(text.contains("MODE_ACTIVE = 64'd2;"));
        assert!(text.contains("mode_field.configure(this, 2, 0, \"RW\", 1, 64'h2, 1, 0, 0)"));
        assert!(text.contains("status_field.configure(this, 4, 8, \"RO\", 1, 64'h0, 1, 0, 0)"));
        assert!(!text.contains("uvm_reg_field value_field;"));
    }
    assert_eq!(texts.first(), texts.get(1));
}
