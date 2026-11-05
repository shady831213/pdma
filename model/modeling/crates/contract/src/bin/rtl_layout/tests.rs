use super::*;
use hw_model_schema::{bits_schema, record_schema, register_schema};

bits_schema! { pub bits Flags(u32) { enabled: 0..=0 } }
record_schema! { pub record Packet { endian: Little, fields: { flags: bits(Flags) } } }
register_schema! {
    pub register Control {
        width: 4, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 1, fields: Flags,
    }
}
mod records {
    hw_model_schema::record_schema! {
        pub record Same { endian: Little, fields: { value: u32 } }
    }
}
mod registers {
    hw_model_schema::register_schema! {
        pub register Same {
            width: 4, endian: Little, access: ReadWrite,
            reset: Zero(0), writable_mask: 0xffff_ffff, fields: {},
        }
    }
}

#[test]
fn shared_definition_emits_once_before_both_domains() -> Result<(), Box<dyn Error>> {
    // Given: both domains depend on the same bits definition.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<Control>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    // When: one generation request routes the unified dependency graph.
    emit_schema(&schema, &mut bytes)?;
    // Then: the shared package exists once and precedes both dependent packages.
    let text = String::from_utf8(bytes)?;
    let shared = text
        .find("pub package FlagsLayout")
        .ok_or("missing flags")?;
    assert_eq!(text.matches("pub package FlagsLayout").count(), 1);
    assert!(
        text.find("pub package PacketLayout")
            .ok_or("missing packet")?
            < text
                .find("pub package ControlLayout")
                .ok_or("missing control")?
    );
    for name in ["PacketLayout", "ControlLayout"] {
        assert!(
            shared
                < text
                    .find(&format!("pub package {name}"))
                    .ok_or("missing dependent")?
        );
    }
    assert!(text.contains("const DECLARED_MASK: u64 = 64'h0000000000000001;"));
    Ok(())
}

#[test]
fn cross_domain_name_conflict_fails_before_any_write() -> Result<(), Box<dyn Error>> {
    // Given: distinct source identities whose target package names collide.
    let mut set = SchemaSet::new();
    set.add::<records::Same>();
    set.add::<registers::Same>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    // When: preparation checks the complete output namespace.
    let error = emit_schema(&schema, &mut bytes).expect_err("duplicate package must fail");
    // Then: collision is diagnosed without writing even the file preamble.
    assert!(error.to_string().contains("target symbol collision"));
    assert!(bytes.is_empty());
    Ok(())
}
