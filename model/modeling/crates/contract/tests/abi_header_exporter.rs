//! Requires the public PDMA C-header exporter composition surface.

use core::convert::Infallible;

use hardware_interface_schema::export::{ArtifactSink, Exporter, InMemoryArtifactSink};
use pdma_model_contract::abi::header::{PdmaCHeaderExporter, render_c_header};

const LEGACY_HEADER: &[u8] = include_bytes!("../../../include/pdma_model_abi.h");

#[test]
fn pdma_local_exporter_when_emitting_to_memory_matches_the_legacy_full_header() {
    // Given: a generic in-memory sink and the legacy renderer/header bytes as independent oracles.
    let mut sink = InMemoryArtifactSink::new();
    assert_generic_sink_contract(&sink);
    let legacy = render_c_header();
    assert_eq!(legacy.as_bytes(), LEGACY_HEADER);
    let exporter = PdmaCHeaderExporter::new();

    // When: the PDMA-local exporter composes and emits its complete C-header artifact.
    exporter
        .export(&(), &mut sink)
        .expect("fixed PDMA C-header export must be infallible");

    // Then: one owned artifact exactly preserves the legacy header rather than deriving its oracle.
    let artifacts = sink.into_artifacts();
    let [artifact] = artifacts.as_slice() else {
        panic!("PDMA exporter must emit exactly one complete C-header artifact");
    };
    assert_eq!(artifact.logical_name, "pdma_model_abi.h");
    assert_eq!(artifact.media_type, "text/x-c; charset=utf-8");
    assert_eq!(artifact.contents, LEGACY_HEADER);
}

const fn assert_generic_sink_contract<Sink>(sink: &Sink)
where
    Sink: ArtifactSink<Error = Infallible>,
{
    let _ = sink;
}
