//! Generates C comments from schema declaration descriptions.
//!
//! Run: `cargo run -p hw-model-schema --features export-c-header --example documented_schema`.
//! Definitions describe reusable types; placements may supply context-specific
//! descriptions. These annotations change documentation, not the wire layout.

use std::io::{self, Write};

use hw_model_schema::{
    export::backends::c_header::{RecordCHeaderExporter, RegisterMapCHeaderExporter},
    export::{Exporter, InMemoryArtifactSink},
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
    RecordCHeaderExporter::new("request.h", "copy")
        .export(&CopyRequest::SPEC.resolve()?, &mut sink)?;
    RegisterMapCHeaderExporter::new("channel.h", "channel")
        .export(&Channel::resolve()?, &mut sink)?;
    let mut output = io::stdout().lock();
    output.write_all(b"#include <stdint.h>\n\n")?;
    for artifact in sink.artifacts() {
        output.write_all(&artifact.contents)?;
        output.write_all(b"\n")?;
    }
    Ok(())
}
