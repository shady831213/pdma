use hardware_interface_runtime::{AccessKind, AccessOrigin, RegisterObservation};
use pdma_model_host::register::PdmaClaimResult;
use std::error::Error;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
type ExpectedObservation = (AccessKind, u32, usize, &'static [u8], &'static [u8]);

pub fn observations(result: &PdmaClaimResult) -> TestResult<&[RegisterObservation]> {
    match result {
        PdmaClaimResult::Notification { observations, .. } => Ok(observations),
        PdmaClaimResult::Empty | PdmaClaimResult::Shutdown => {
            Err("expected notification transcript".into())
        }
    }
}

pub fn assert_exact(actual: &[RegisterObservation]) -> TestResult<()> {
    let expected: [ExpectedObservation; 11] = [
        (AccessKind::Write, 0x220, 4, &[9, 0, 0, 0], &[]),
        (AccessKind::Read, 0x224, 4, &[], &[1, 0, 0, 0]),
        (AccessKind::Read, 0x240, 8, &[], &[0; 8]),
        (AccessKind::Read, 0x248, 8, &[], &[0; 8]),
        (AccessKind::Read, 0x250, 4, &[], &[0; 4]),
        (AccessKind::Read, 0x254, 4, &[], &[0x4b, 0, 0, 0]),
        (AccessKind::Read, 0x258, 4, &[], &[1, 0, 0, 0]),
        (AccessKind::Read, 0x25c, 4, &[], &[2, 0, 0, 0]),
        (
            AccessKind::Read,
            0x260,
            8,
            &[],
            &[0x29, 0, 0, 0, 0, 0, 0, 0],
        ),
        (AccessKind::Read, 0x268, 8, &[], &[0; 8]),
        (AccessKind::Read, 0x270, 8, &[], &[0; 8]),
    ];
    if actual.len() != expected.len() {
        return Err("claim transcript length differs".into());
    }
    for (sequence, (item, (kind, offset, width, request, response))) in
        actual.iter().zip(expected).enumerate()
    {
        if item.sequence != sequence as u64
            || item.origin != AccessOrigin::Direct
            || item.kind != kind
            || item.offset != offset
            || item.width != width
            || item.request_bytes != request
            || item.response_bytes != response
        {
            return Err(format!("claim transcript differs at {sequence}: {item:?}").into());
        }
    }
    Ok(())
}

pub fn is_notification(result: &PdmaClaimResult) -> bool {
    matches!(result, PdmaClaimResult::Notification { .. })
}

pub fn label(result: &PdmaClaimResult) -> &'static str {
    match result {
        PdmaClaimResult::Notification { .. } => "message",
        PdmaClaimResult::Empty => "empty",
        PdmaClaimResult::Shutdown => "shutdown",
    }
}
