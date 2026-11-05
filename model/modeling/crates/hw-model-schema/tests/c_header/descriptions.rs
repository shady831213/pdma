use crate::output_support::MemoryOutput;
use hw_model_schema::{
    BitsFieldDefinition, BitsFieldSpec, Documentation, Endian, FieldSpec, RecordSpec, ScalarKind,
    register::{RegisterDefinition, RegisterSpec},
    register_map::{RegisterMapSpec, RepeatedRegisterBank},
};

use super::support::{TestResult, compile_fragments, render_map, render_record};

const DETAILS: Documentation = Documentation {
    short_description: Some("Transfer length"),
    description: Some("Number of bytes.\nZero is allowed."),
};
const FIELDS: [FieldSpec; 1] =
    [FieldSpec::new(0, "length", 0, ScalarKind::U32).with_documentation(DETAILS)];
const RECORD: RecordSpec = RecordSpec::new(4, 4, &FIELDS).with_documentation(Documentation {
    short_description: Some("Copy request"),
    description: None,
});
const BITS: [BitsFieldSpec; 1] = [
    BitsFieldSpec::place(0, "ready", 0, BitsFieldDefinition::new(1)).with_documentation(
        Documentation {
            short_description: Some("Ready flag"),
            description: Some("Set after completion."),
        },
    ),
];
const STATUS: RegisterDefinition = RegisterDefinition::new("Status", 4, Endian::Little)
    .with_fields(&BITS)
    .with_documentation(DETAILS);
const REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::place(0, "status", 0, STATUS)
    .with_documentation(Documentation {
        short_description: Some("Channel status"),
        description: None,
    })];
const BANK_REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::place(1, "status", 0, STATUS)
    .with_documentation(Documentation {
        short_description: Some("Channel status"),
        description: None,
    })];
const BANKS: [RepeatedRegisterBank<u16>; 1] =
    [
        RepeatedRegisterBank::new("channels", 4, 2, 4, &BANK_REGISTERS).with_documentation(
            Documentation {
                short_description: Some("Channel bank"),
                description: None,
            },
        ),
    ];
const MAP: RegisterMapSpec<u16> =
    RegisterMapSpec::new(12, &REGISTERS, &BANKS).with_documentation(Documentation {
        short_description: Some("Device registers"),
        description: None,
    });

#[test]
fn record_comments_follow_the_described_declaration() -> TestResult {
    // Given: record and member descriptions independent of their layout.
    let exporter = ("record.h", "request");
    // When: the resolved record is rendered by the public exporter.
    let artifact = render_record(exporter.0, exporter.1, &RECORD)?;
    let text = std::str::from_utf8(&artifact.bytes)?;
    // Then: root and member comments precede their unchanged C declarations.
    assert!(
        text.find("Copy request").expect("root comment")
            < text.find("#define C_REQQUEST_SIZE").expect("root constant")
    );
    assert!(text.contains("Transfer length\n * Number of bytes.\n * Zero is allowed."));
    assert!(
        text.find("Transfer length").expect("field comment")
            < text
                .find("#define C_REQQUEST_C_LENGTH_OFFSET")
                .expect("field constant")
    );
    compile_fragments(
        "description-record",
        &[&artifact.bytes],
        b"int main(void) { return C_REQQUEST_SIZE != 4; }\n",
    )?;
    Ok(())
}

#[test]
fn register_comments_preserve_placement_fallback_bits_and_banks() -> TestResult {
    // Given: documented register placements with an inherited detailed description.
    let exporter = ("map.h", "device");
    // When: fixed registers and repeated banks are exported together.
    let artifact = render_map(exporter.0, exporter.1, &MAP.resolve()?)?;
    let text = std::str::from_utf8(&artifact.bytes)?;
    // Then: every ownership level and bit field remains documented.
    assert!(text.contains("Device registers"));
    assert!(text.contains("Channel bank"));
    assert_eq!(text.matches("Channel status").count(), 2);
    assert_eq!(text.matches("Number of bytes.").count(), 2);
    assert_eq!(text.matches("Ready flag").count(), 2);
    assert!(text.contains("ready"));
    assert!(!text.contains("Transfer length"));
    compile_fragments(
        "description-map",
        &[&artifact.bytes],
        b"int main(void) { return C_DEVICE_SIZE != 12; }\n",
    )?;
    Ok(())
}

#[test]
fn hostile_description_remains_a_comment_in_strict_c11() -> TestResult {
    // Given: comment delimiters, line splicing and trigraphs in literal documentation.
    let hostile = RECORD.with_documentation(Documentation {
        short_description: Some("Header /* nested */"),
        description: Some("*/\n#error injected\n/*\n*\\\n/\n*??/\n/"),
    });
    // When: the described schema is rendered as a real C header.
    let artifact = render_record("hostile.h", "request", &hostile)?;
    // Then: the text cannot escape its comments, and generated access still works.
    compile_fragments(
        "description-hostile",
        &[&artifact.bytes],
        br"
int main(void) {
    uint8_t bytes[C_REQQUEST_SIZE] = {0};
    C_REQQUEST_STORE_C_LENGTH(bytes, UINT32_C(42));
    return C_REQQUEST_LOAD_C_LENGTH(bytes) != 42;
}
",
    )?;
    Ok(())
}

#[test]
fn reusable_bits_documentation_is_emitted_once_in_each_c_backend() -> TestResult {
    use hw_model_schema::{
        analysis::{AnalyzedSchema, Bindings},
        ast::SchemaSet,
        export::backends::c_header::{CRecordBackend, CRegisterBackend},
    };
    hw_model_schema::bits_schema! {
        #[short_description = "Reusable flags summary"]
        #[description = "Flags details.\nSecond line */ remains text."]
        pub bits DocumentedFlags(u8) {
            #[short_description = "Ready summary"]
            #[description = "Ready details."]
            ready: 0..=0,
        }
    }
    // Given: a standalone bits definition shared by both C source domains.
    let mut set = SchemaSet::new();
    set.add::<DocumentedFlags>();
    let graph = set.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let mut record = MemoryOutput::new();
    let mut register = MemoryOutput::new();
    let id = schema
        .definition(hw_model_schema::ast::DefinitionId::of::<DocumentedFlags>())
        .ok_or("missing flags")?
        .id
        .clone();
    // When: each public backend renders the same documented source definition.
    record.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((&schema, ()))
            .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        |data| CRecordBackend::new(data).with_name(id.clone(), "flags"),
    )?;
    register.generate(&schema, CRegisterBackend::new().with_name(id, "flags"))?;
    // Then: each fragment appears exactly once and escaped output compiles strictly.
    for (label, sink) in [("bits-doc-record", record), ("bits-doc-register", register)] {
        let artifact = sink.outputs().first().ok_or("missing bits artifact")?;
        let text = std::str::from_utf8(&artifact.bytes)?;
        for fragment in [
            "Reusable flags summary",
            "Flags details.",
            "Second line * / remains text.",
            "Ready summary",
            "Ready details.",
        ] {
            assert_eq!(text.matches(fragment).count(), 1, "{label}: {fragment}");
        }
        compile_fragments(
            label,
            &[&artifact.bytes],
            b"int main(void) { return C_FLAGS_SIZE != 1; }\n",
        )?;
    }
    Ok(())
}
