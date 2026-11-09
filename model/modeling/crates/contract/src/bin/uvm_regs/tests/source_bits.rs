use super::super::*;
use hw_model_schema::{
    analysis::Pass,
    bits_schema, register_schema,
    transform::{Inline, Outline},
};

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
fn source_bits_and_inline_bits_preserve_register_fields() {
    // Given one reusable bits source and its equivalent inline/outlined forms.
    let mut definitions = SchemaSet::new();
    definitions.add::<Shared>();
    let mut original = SchemaSet::new();
    original.add::<Shared>();
    let graph = original.resolve().unwrap();
    let inline = Inline::new().run(&graph).unwrap();
    let outlined = Outline::new()
        .run(Inline::new().run(&graph).unwrap())
        .unwrap();
    // When rendering each real source form through the UVM backend.
    let texts: Vec<_> = [definitions, inline, outlined]
        .into_iter()
        .map(|source| render(source).unwrap())
        .collect();
    // Then the source edge changes no field, enum, reset or permission output.
    for text in &texts {
        assert!(text.contains("uvm_reg_field mode_field;"));
        assert!(text.contains("MODE_ACTIVE = 64'd2;"));
        assert!(text.contains("mode_field.configure(this, 2, 0, \"RW\", 1, 64'h2, 1, 0, 0)"));
        assert!(text.contains("status_field.configure(this, 4, 8, \"RO\", 1, 64'h0, 1, 0, 0)"));
        assert!(!text.contains("uvm_reg_field value_field;"));
    }
    assert!(texts.windows(2).all(|pair| pair.first() == pair.get(1)));
}
