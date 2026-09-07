#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "checked unit-test fixtures"
)]

use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::ResourceConfig;

use super::{PdmaCommandPipeline, PipelineSlot};
use crate::device::DeviceCore;

mod adverse;
mod no_replay;
mod parity;
mod support;

#[tokio::test(flavor = "current_thread")]
async fn drive_policy_parity() {
    parity::drive_policy_parity().await;
}

#[test]
fn no_replay() {
    no_replay::no_replay();
}

#[test]
#[should_panic(expected = "command pipeline exhausted before terminal result")]
fn compatibility_step_rejects_exhausted_retained_pipeline() {
    // Given: an already-terminal command pipeline is retained in the device slot.
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    );
    let mut pipeline = PdmaCommandPipeline::new();
    loop {
        match pipeline.step(&mut core) {
            StepOutcome::Progressed => {}
            StepOutcome::Complete(_) => break,
            StepOutcome::Pending(reason) => panic!("idle pipeline stalled: {reason:?}"),
            StepOutcome::Exhausted => panic!("pipeline exhausted before initial completion"),
        }
    }
    core.command_pipeline = PipelineSlot {
        pipeline: Some(pipeline),
    };

    // When: the compatibility driver tries to advance that retained pipeline.
    let _ = core.step();

    // Then: the should-panic contract rejects spinning on a non-interaction outcome.
}
