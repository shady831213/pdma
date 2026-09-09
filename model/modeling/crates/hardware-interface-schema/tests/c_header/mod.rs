//! Generic C11 fragment exporter integration tests.

#![cfg(feature = "export-c-header")]

#[allow(missing_docs)]
pub mod support;

mod descriptions;
mod ordered_descriptions;

use support::{
    COLLISION_C_MAIN, HAPPY_C_MAIN, NESTED_C_MAIN, PREFIX_C_MAIN, TestResult, compile_fragments,
    render,
};

use hardware_interface_schema::{
    Endian, FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    export::backends::c_header::{
        CHeaderExportError, RecordCHeaderExporter, RegisterMapCHeaderExporter,
    },
    export::{Artifact, ArtifactSink, Exporter},
    register::{Access, RegisterSpec, ResetKind},
    register_map::{RegisterMapSpec, RepeatedRegisterBank},
};

const RECORD_GOLDEN: &[u8] = include_bytes!("../golden/c_header_record.h");
const MAP_GOLDEN: &[u8] = include_bytes!("../golden/c_header_register_map.h");
const COLLISION_RECORD_GOLDEN: &[u8] = include_bytes!("../golden/c_header_collision_record.h");
const COLLISION_MAP_GOLDEN: &[u8] = include_bytes!("../golden/c_header_collision_map.h");

include!("record_tests.rs");
include!("register_map_tests.rs");
