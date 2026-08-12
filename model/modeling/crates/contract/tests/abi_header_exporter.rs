//! Requires the public PDMA C-header exporter composition surface.

use hardware_interface_schema::export::{ArtifactSink, Exporter, InMemoryArtifactSink};
use pdma_model_contract::abi::header::{
    HeaderRenderError, PdmaCHeaderExporter, PdmaHeaderSchemas, render_c_header,
};

const LEGACY_HEADER: &[u8] = include_bytes!("../../../include/pdma_model_abi.h");

#[test]
fn pdma_local_exporter_when_emitting_to_memory_matches_the_legacy_full_header() {
    // Given: a generic in-memory sink and the legacy renderer/header bytes as independent oracles.
    let mut sink = InMemoryArtifactSink::<HeaderRenderError>::new();
    assert_generic_sink_contract(&sink);
    let legacy = render_c_header().expect("canonical schemas must resolve");
    assert_eq!(legacy.as_bytes(), LEGACY_HEADER);
    let exporter = PdmaCHeaderExporter::new();
    let schemas = PdmaHeaderSchemas::resolve().expect("canonical schemas must resolve");

    // When: the PDMA-local exporter composes and emits its complete C-header artifact.
    exporter
        .export(&schemas, &mut sink)
        .expect("canonical schemas must resolve");

    // Then: one owned artifact exactly preserves the legacy header rather than deriving its oracle.
    let artifacts = sink.into_artifacts();
    let [artifact] = artifacts.as_slice() else {
        panic!("PDMA exporter must emit exactly one complete C-header artifact");
    };
    assert_eq!(artifact.logical_name, "pdma_model_abi.h");
    assert_eq!(artifact.media_type, "text/x-c; charset=utf-8");
    assert_eq!(artifact.contents, LEGACY_HEADER);
}

#[test]
fn pdma_exporter_when_given_resolved_schemas_uses_the_typed_graph_surface() {
    // Given: the public canonical PDMA schema bundle is resolved once.
    let schemas = PdmaHeaderSchemas::resolve().expect("canonical schemas must resolve");
    let exporter = PdmaCHeaderExporter::new();

    // When: the exporter renders that typed graph directly.
    let rendered = exporter.render(&schemas);

    // Then: the resolved-graph surface retains the exact checked-in artifact.
    assert_eq!(rendered.as_bytes(), LEGACY_HEADER);
}

const fn assert_generic_sink_contract<Sink>(sink: &Sink)
where
    Sink: ArtifactSink<Error = HeaderRenderError>,
{
    let _ = sink;
}
