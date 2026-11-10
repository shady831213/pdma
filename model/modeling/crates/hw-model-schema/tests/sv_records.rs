//! Generic byte-record rendering and writer failures.
#![cfg(feature = "export-sv")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{backends::system_verilog::SvRecordBackend, prepare_entries, record},
};
fn render(set: SchemaSet) -> Result<String, Box<dyn std::error::Error>> {
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut backend = SvRecordBackend::new();
    let mut bytes = b"package schema_records;\n".to_vec();
    for entry in prepare_entries(
        &mut backend,
        record::entries(&schema)
            .filter(|v| v.definition_origin() == hw_model_schema::ast::Origin::Named),
    )? {
        entry.emit(&mut backend, &mut bytes)?;
    }
    bytes.extend_from_slice(b"endpackage\n");
    Ok(String::from_utf8(bytes)?)
}
use hw_model_schema::{bits_schema, record_schema};

record_schema! {
    pub record Narrow { alignment: 8, endian: Little, fields: { prefix: u8, value: u32 } }
}
record_schema! {
    pub record Wide { alignment: 8, endian: Little, fields: { prefix: u64, value: u32 } }
}
record_schema! {
    pub record Network { alignment: 8, endian: Big, fields: { value: u32 } }
}

bits_schema! { pub bits FirstState(u8) { flag: 0..=1 => enum { variants: { Idle = 0, Busy = 1 } } } }
bits_schema! { pub bits SecondState(u8) { flag: 0..=1 => enum { variants: { Idle = 2, Busy = 3 } } } }

record_schema! {
    pub record InlineChoice { endian: Little, fields: {
        tag: enum(u8) { variants: { First = 1, Second = 2 } },
        payload: tagged_union { tag: self.tag, cases: {
            First { state: FirstState },
            Second { state: SecondState },
        } },
    } }
}

#[test]
fn inline_union_values_have_case_qualified_names() {
    let mut definitions = SchemaSet::new();
    definitions.add::<InlineChoice>();
    definitions.add::<Network>();
    definitions.add::<Narrow>();
    let text = render(definitions).unwrap();
    assert!(text.contains("INLINECHOICE_PAYLOAD_FIRST_STATE_FLAG_BUSY = 64'd1;"));
    assert!(text.contains("INLINECHOICE_PAYLOAD_SECOND_STATE_FLAG_BUSY = 64'd3;"));
    assert!(text.contains("InlineChoice_get_payload"));
    if let Some(path) = std::env::var_os("SCHEMA_SV_INLINE_FIXTURE") {
        let bench = "module inline_union_metadata;\n import schema_records::*;\n initial begin\n InlineChoice_t image = '0;\n Network_t network = '0;\n Narrow_t narrow = '0;\n if (INLINECHOICE_PAYLOAD_FIRST_STATE_FLAG_BUSY != 1 || INLINECHOICE_PAYLOAD_SECOND_STATE_FLAG_BUSY != 3) $fatal(1, \"inline enum metadata\");\n InlineChoice_set_payload(image, 8'h3);\n if (InlineChoice_get_payload(image) != 3 || image[15:8] != 3) $fatal(1, \"union envelope codec\");\n Network_set_value(network,32'h12345678);\n Narrow_set_prefix(narrow,8'ha5);\n Narrow_set_value(narrow,32'hdeadbeef);\n if (network[7:0] != 8'h12 || network[31:24] != 8'h78 || Network_get_value(network) != 32'h12345678 || narrow[7:0] != 8'ha5 || narrow[32 +: 32] != 32'hdeadbeef || Narrow_get_value(narrow) != 32'hdeadbeef) $fatal(1, \"record byte order\");\n $display(\"INLINE_UNION_PASS\");\n $finish;\n end\nendmodule\n";
        std::fs::write(path, format!("{text}{bench}")).unwrap();
    }
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

#[test]
fn writer_failure_propagates_from_output_rendering() {
    // Given an analyzed fixture and its complete output size.
    let mut definitions = SchemaSet::new();
    definitions.add::<InlineChoice>();
    let schema = AnalyzedSchema::analyze(definitions, &Bindings::new()).unwrap();
    let mut backend = SvRecordBackend::new();
    let outputs = prepare_entries(&mut backend, record::entries(&schema)).unwrap();
    for output in outputs {
        let mut complete = Vec::new();
        output.emit(&mut backend, &mut complete).unwrap();
        for capacity in [0, complete.len() / 2, complete.len() - 1] {
            let mut storage = vec![0; capacity];
            // When the actual output writer runs out of capacity.
            let error = output
                .emit(&mut backend, &mut storage.as_mut_slice())
                .unwrap_err();
            // Then the writer error reaches the caller, including late writes.
            assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
        }
    }
}
