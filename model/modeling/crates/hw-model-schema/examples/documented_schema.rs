//! Generates C comments from schema declaration descriptions.
//!
//! Run: `cargo run -p hw-model-schema --features export-c-header --example documented_schema`.
//! Definitions describe reusable types; placements may supply context-specific
//! descriptions. These annotations change documentation, not the wire layout.

use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::ast::{DefinitionId, SchemaSet};
use std::io::{self, Write};

use hw_model_schema::{
    export::backends::c_header::{CRecordAnalysisPass, CRecordBackend, CRegisterBackend},
    export::{InMemoryArtifactSink, generate},
    record_schema, register_map, register_schema,
};

record_schema! {
    #[short_description = "DMA copy request"]
    pub record CopyRequest {
        endian: Little,
        fields: {
            #[short_description = "Transfer length"]
            #[description = "Number of bytes to copy.\nZero performs no memory transfer."]
            length: u32,
        },
    }
}

register_schema! {
    #[short_description = "Enable register"]
    #[description = "Only bit zero is writable."]
    pub register Enable {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 1,
        fields: {
            #[short_description = "Enabled"]
            #[description = "Allows processing while set."]
            enabled: 0..1,
        },
    }
}

register_map! {
    #[short_description = "Channel registers"]
    pub map Channel {
        alignment: 4,
        placements: [
            // This placement overrides the type's short description. Its omitted
            // long description still inherits "Only bit zero is writable."
            #[short_description = "Command processing enable"]
            pub enable: Enable,
        ],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve metadata once, then export to an injected sink. No output files are
    // created: this program writes the resulting headers to standard output.
    let mut sink = InMemoryArtifactSink::new();
    let mut records = SchemaSet::new();
    records.add::<CopyRequest>();
    let schema = AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<CopyRequest>())
        .ok_or("missing copy")?
        .id
        .clone();
    hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(CRecordAnalysisPass)?
        .generate(
            |data| CRecordBackend::new(data).with_name(id, "copy"),
            &mut sink,
        )?;
    let schema = AnalyzedSchema::register_map("channel.h", &Channel::resolve()?)?;
    generate(
        &schema,
        CRegisterBackend::new().with_name(
            schema
                .view("channel.h")
                .ok_or("missing channel")?
                .id
                .clone(),
            "channel",
        ),
        &mut sink,
    )?;
    let mut output = io::stdout().lock();
    output.write_all(b"#include <stdint.h>\n\n")?;
    for artifact in sink.artifacts() {
        output.write_all(&artifact.contents)?;
        output.write_all(b"\n")?;
    }
    Ok(())
}
