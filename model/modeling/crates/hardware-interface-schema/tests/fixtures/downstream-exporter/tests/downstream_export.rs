//! Exercises the standalone downstream export surface through its public API.

use downstream_exporter::{
    TelemetryExporter, TelemetryMap, TelemetryRecord, TelemetrySink, composable_output_line,
};
use his::{export::Exporter, register_map::RegisterLocation};

#[test]
fn downstream_export_when_schema_is_declared_through_renamed_facade() {
    // Given
    let exporter = TelemetryExporter;
    let mut sink = TelemetrySink::default();

    // When
    let result = exporter.export(&TelemetryMap::SPEC, &mut sink);

    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(TelemetryRecord::SPEC.size(), 12);
    assert_eq!(TelemetryMap::SPEC.size(), 128);
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
    assert_eq!(sink.artifacts().len(), 1);
    let composable = composable_output_line();
    assert_eq!(composable.as_deref(), Some("nested=[44, 33, 22, 11, 00, 00, 00, 00, 00, 00, 00, 00, 66, 55, 00, 00];le=[01, 00, 00, 00];be=[12, 34];left=0x11223344;enable=1;status=0x1234"));
    println!("{}", composable.as_deref().unwrap_or("missing composable output"));
}
