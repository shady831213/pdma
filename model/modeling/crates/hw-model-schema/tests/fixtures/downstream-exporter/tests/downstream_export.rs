//! Exercises the standalone downstream export surface through its public API.

use downstream_exporter::{
    TelemetryMap, TelemetryRecord, TelemetrySink, composable_output_line, export_resolved,
};
use his::register_map::RegisterLocation;

#[test]
fn downstream_export_when_schema_is_declared_through_renamed_facade()
-> Result<(), Box<dyn std::error::Error>> {
    // Given
    let mut sink = TelemetrySink::default();

    // When
    let result = export_resolved(&mut sink);

    // Then
    result?;
    assert_eq!(TelemetryRecord::SPEC.size(), 12);
    assert_eq!(TelemetryMap::EXTENT, 128);
    assert!(matches!(
        TelemetryMap::locate(4),
        Some(RegisterLocation::Fixed { offset: 4, .. })
    ));
    assert!(matches!(
        TelemetryMap::locate(32),
        Some(RegisterLocation::Repeated {
            instance: 0,
            offset: 32,
            ..
        })
    ));
    assert!(matches!(
        TelemetryMap::locate(64),
        Some(RegisterLocation::Repeated {
            instance: 1,
            offset: 64,
            ..
        })
    ));
    let [artifact] = sink.artifacts() else {
        panic!("expected one application artifact");
    };
    assert_eq!(artifact.logical_name, "acme-telemetry-layout.txt");
    assert_eq!(artifact.media_type, "text/vnd.acme.telemetry-layout");
    assert_eq!(
        artifact.contents,
        b"record_size=16;record=DownPair.left@0+8,DownPair.left.sequence@0+4,DownPair.left.flags@4+2,DownPair.right@8+8,DownPair.right.sequence@8+4,DownPair.right.flags@12+2;map_size=128;registers=controller@4,status@12,lane[0].publish@32,lane[0].fault@40,lane[1].publish@64,lane[1].fault@72"
    );
    let composable = composable_output_line();
    assert_eq!(
        composable.as_deref(),
        Some(
            "nested=[44, 33, 22, 11, 00, 00, 00, 00, 00, 00, 00, 00, 66, 55, 00, 00];le=[05, 00, 00, 00];be=[12, 34];bits=[05];left=0x11223344;enable=true;mode=2;status=0x1234"
        )
    );
    println!(
        "{}",
        composable.as_deref().unwrap_or("missing composable output")
    );
    Ok(())
}

#[test]
fn record_renderer_exports_without_a_register_backend() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a record-only graph and the downstream record renderer.
    let mut records = his::ast::SchemaSet::new();
    records.add_record::<downstream_exporter::DownPair>();
    let schema = his::analysis::AnalyzedSchema::from_graph(
        &records.resolve()?,
        &his::analysis::Bindings::new(),
    )?;
    // When: only record-domain generation is invoked.
    let reports = capture(&schema, downstream_exporter::TelemetryRecordRenderer)?;
    // Then: the artifact contains the independently expected record report alone.
    let reports: Vec<_> = reports
        .iter()
        .filter(|report| !report.contents.is_empty())
        .collect();
    let [artifact] = reports.as_slice() else {
        panic!("one nonempty report expected");
    };
    assert_eq!(artifact.logical_name, "records.txt");
    assert_eq!(artifact.contents, b"record_size=16;record=DownPair.left@0+8,DownPair.left.sequence@0+4,DownPair.left.flags@4+2,DownPair.right@8+8,DownPair.right.sequence@8+4,DownPair.right.flags@12+2;");
    Ok(())
}

#[test]
fn register_renderer_exports_without_a_record_backend() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a register-only graph and the downstream register renderer.
    let mut registers = his::ast::SchemaSet::new();
    registers.add::<TelemetryMap>();
    let schema = his::analysis::AnalyzedSchema::from_graph(
        &registers.resolve()?,
        &his::analysis::Bindings::new(),
    )?;
    // When: only register-domain generation is invoked.
    let reports = capture(&schema, downstream_exporter::TelemetryRegisterRenderer)?;
    // Then: the artifact contains the independently expected register report alone.
    let reports: Vec<_> = reports
        .iter()
        .filter(|report| !report.contents.is_empty())
        .collect();
    let [artifact] = reports.as_slice() else {
        panic!("one nonempty report expected");
    };
    assert_eq!(artifact.logical_name, "registers.txt");
    assert_eq!(artifact.contents, b"map_size=128;registers=controller@4,status@12,lane[0].publish@32,lane[0].fault@40,lane[1].publish@64,lane[1].fault@72");
    Ok(())
}

fn capture<B: his::export::value::Backend>(
    schema: &his::analysis::AnalyzedSchema,
    mut backend: B,
) -> Result<Vec<downstream_exporter::TelemetryReport>, his::export::ExportError<B::Error>> {
    let mut reports = Vec::new();
    for unit in his::export::prepare(schema, &mut backend)? {
        let mut contents = Vec::new();
        unit.emit(&mut backend, &mut contents)
            .map_err(his::export::ExportError::Render)?;
        reports.push(downstream_exporter::TelemetryReport {
            logical_name: unit.metadata().logical_name.clone(),
            media_type: unit.metadata().media_type.clone(),
            contents,
        });
    }
    Ok(reports)
}
