//! Exercises the standalone downstream export surface through its public API.

use downstream_exporter::{
    TelemetryMap, TelemetryRecord, TelemetrySink, composable_output_line, export_resolved,
};
use his::register_map::RegisterLocation;

#[test]
fn downstream_export_when_schema_is_declared_through_renamed_facade() {
    // Given
    let mut sink = TelemetrySink::default();

    // When
    let result = export_resolved(&mut sink);

    // Then
    assert_eq!(result, Some(()));
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
    assert_eq!(
        artifact.media_type,
        "text/vnd.acme.telemetry-layout"
    );
    assert_eq!(
        artifact.contents,
        b"record_size=16;record=DownPair.left@0+8,DownPair.left.sequence@0+4,DownPair.left.flags@4+2,DownPair.right@8+8,DownPair.right.sequence@8+4,DownPair.right.flags@12+2;map_size=128;registers=controller@4,status@12,lane[0].publish@32,lane[0].fault@40,lane[1].publish@64,lane[1].fault@72"
    );
    let composable = composable_output_line();
    assert_eq!(composable.as_deref(), Some("nested=[44, 33, 22, 11, 00, 00, 00, 00, 00, 00, 00, 00, 66, 55, 00, 00];le=[05, 00, 00, 00];be=[12, 34];bits=[05];left=0x11223344;enable=true;mode=2;status=0x1234"));
    println!("{}", composable.as_deref().unwrap_or("missing composable output"));
}
