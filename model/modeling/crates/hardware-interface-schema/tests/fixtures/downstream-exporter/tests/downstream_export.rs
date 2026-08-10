//! Exercises the standalone downstream export surface through its public API.

use downstream_exporter::{TelemetryExporter, TelemetryMap, TelemetryRecord, TelemetrySink};
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
}
