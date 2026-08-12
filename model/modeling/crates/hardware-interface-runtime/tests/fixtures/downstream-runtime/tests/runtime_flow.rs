//! Exercises the locked standalone consumer through its public API.

use downstream_runtime::run_flow;

#[tokio::test(flavor = "current_thread")]
async fn runtime_flow_when_consumed_through_renamed_facade() {
    // Given
    let expected_happy = "memory=64:first=0x11:last=0xee;register=write@0x20:[de,ad,be,ef],read@0x10:[78,56,34,12];order=memory-write,memory-read,register-write,register-read,main-before,handler-start,handler-finish,main-after;deliveries=1;trace-sequences=0..7";
    let expected_adverse = "adverse memory=Unmapped{address=0x5080},CrossesRegion{address=0x503f,length=2,region=0x5000..0x5040};destination_hash=120;store_hash=5835;interrupt=UnregisteredVector(9);handler_entries=0;cpu_entries=0;lifecycle_events=0;source_consumed=1;source_drops=1;handler_drops=1;joined=true";

    // When
    let report = run_flow().await;

    // Then
    let report = report.unwrap_or_else(|error| panic!("standalone consumer failed: {error}"));
    assert_eq!(report.happy_line, expected_happy);
    assert_eq!(report.adverse_line, expected_adverse);
}
