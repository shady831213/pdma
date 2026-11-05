//! Consumers can render a graph without knowing how roots were discovered.
#![cfg(feature = "export-c-header")]
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalysisError, AnalyzedSchema, Bindings};
use hw_model_schema::export::backends::c_header::{CHeaderExportError, CRecordBackend};
use hw_model_schema::{
    Endian, RecordSpec, RegisterDefinition,
    ast::{Definition, DefinitionId, SchemaSet},
    export::ExportError,
};
use hw_model_schema::{record_schema, union_schema};
use output_support::MemoryOutput;

record_schema! { pub record Payload { alignment: 4, endian: Little, fields: { value: u32 } } }
union_schema! { pub union Message { cases: { Payload(Payload) } } }

mod left {
    hw_model_schema::record_schema! { pub record Entry { endian: Little, fields: { left: u32 } } }
}
mod right {
    hw_model_schema::record_schema! { pub record Entry { endian: Little, fields: { right: u32 } } }
}

#[test]
fn distinct_same_named_types_require_an_explicit_backend_name_override()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two unrelated Rust definitions share a short name.
    let mut set = SchemaSet::new();
    set.add_record::<left::Entry>();
    set.add_record::<right::Entry>();
    let graph = set.resolve()?;
    // When: the default backend would emit the same C prefix twice.
    let mut sink = MemoryOutput::new();
    let prepared = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let c_prepared = hw_model_schema::analysis::Pipeline::new((&prepared, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?;
    let failure = sink.generate_prepared(&c_prepared, CRecordBackend::new);
    // Then: metadata preflight prevents all output; renaming permits both roots.
    let Err(ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    })) = failure
    else {
        return Err("expected name collision".into());
    };
    assert_eq!(first_path, DefinitionId::of::<left::Entry>().path());
    assert_eq!(second_path, DefinitionId::of::<right::Entry>().path());
    assert_eq!((first_index, second_index), (0, 1));
    assert!(sink.outputs().is_empty());
    let mut sink = MemoryOutput::new();
    let renderer = CRecordBackend::new(&c_prepared.input().1).with_name(
        prepared
            .definition(hw_model_schema::ast::DefinitionId::of::<right::Entry>())
            .ok_or("missing right entry")?
            .id
            .clone(),
        "right_entry",
    );
    sink.generate(&prepared, renderer)?;
    assert_eq!(sink.outputs().len(), 2);
    Ok(())
}

#[test]
fn c_header_exports_reachable_records_once() -> Result<(), Box<dyn std::error::Error>> {
    // Given: only the outer union is added to a manual graph.
    let mut set = SchemaSet::new();
    set.add_record::<Message>();
    let graph = set.resolve()?;
    // When: the built-in C backend renders its complete dependency set.
    let mut sink = MemoryOutput::new();
    sink.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((
            &AnalyzedSchema::from_graph(&graph, &Bindings::new())?,
            (),
        ))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        CRecordBackend::new,
    )?;
    // Then: both storage geometry and the referenced record's accessors are present.
    let output = sink
        .outputs()
        .iter()
        .map(|artifact| String::from_utf8_lossy(&artifact.bytes))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(output.matches("#define C_PAYLOAD_SIZE ").count(), 1);
    assert!(output.contains("C_PAYLOAD_LOAD_C_VALUE"));
    assert!(output.contains("#define C_MESSAGE_SIZE "));
    Ok(())
}

struct ZInvalid;

#[test]
fn invalid_record_retains_layout_error_during_preparation() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a valid record precedes an opaque record with invalid alignment.
    let malformed = RecordSpec::named("Invalid", 4, 0, &[], &[]);
    let expected = malformed.resolve().unwrap_err();
    let mut set = SchemaSet::new();
    set.add_record::<Payload>();
    set.add_definition(Definition::opaque::<ZInvalid>(malformed));
    let graph = set.resolve()?;
    assert_eq!(
        graph.definitions().next().unwrap().id(),
        DefinitionId::of::<Payload>()
    );
    // When: preparation resolves the later record before rendering begins.
    let failure = AnalyzedSchema::from_graph(&graph, &Bindings::new());

    // Then: the exact layout cause and defining path survive, with no partial output.
    let Err(AnalysisError::Record { path, source }) = failure else {
        return Err("expected typed record layout error".into());
    };
    assert_eq!(path, DefinitionId::of::<ZInvalid>().path());
    assert_eq!(source, expected);
    Ok(())
}

#[test]
fn graph_register_uses_prepared_register_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a valid record precedes a whole register in the selected graph.
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register::<ZInvalid, u16>(
        RegisterDefinition::new("Control", 4, Endian::Little),
    ));
    let graph = set.resolve()?;
    let mut sink = MemoryOutput::new();
    let mut records = SchemaSet::new();
    records.add_record::<Payload>();
    sink.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((
            &AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::new())?,
            (),
        ))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        CRecordBackend::new,
    )?;

    // When: the common preparer supplies register metadata to the C renderer.
    sink.generate(
        &AnalyzedSchema::from_graph(&graph, &Bindings::new())?,
        hw_model_schema::export::backends::c_header::CRegisterBackend::new(),
    )?;

    // Then: both selected definitions are rendered, including the register width.
    assert_eq!(sink.outputs().len(), 2);
    let register = sink
        .outputs()
        .iter()
        .find(|artifact| artifact.metadata.logical_name == "control")
        .ok_or("register artifact")?;
    assert!(std::str::from_utf8(&register.bytes)?.contains("#define C_CONTROL_WIDTH UINT64_C(4)"));
    Ok(())
}
