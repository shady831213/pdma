#[path = "support/recording.rs"]
mod recording;

use pdma_model_contract::abi::{
    DeviceEpoch, DmaRegion, QueueActivation, QueueRingRegions, RingLayout, RingRecord,
};
use pdma_model_contract::{EventRouting, QueueConfig, QueueId, SoftwareOwnerId};
use recording::{Access, RecordingTransport};

fn activation(
    regions: &[DmaRegion],
) -> Result<QueueActivation, Box<dyn std::error::Error + Send + Sync>> {
    let [submission, completion, event] = regions else {
        return Err("three regions required".into());
    };
    let rings = QueueRingRegions::new(
        RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
        RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
        RingLayout::try_new(*event, 2, RingRecord::Event)?,
    );
    Ok(QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        rings,
    )?)
}

fn expected_queue_transcript() -> Vec<Access> {
    let mut accesses = Vec::new();
    append_status(&mut accesses);
    for command in 4_u32..=9 {
        append_status(&mut accesses);
        accesses.push(Access::Write(0x110, 0_u64.to_le_bytes().to_vec()));
        accesses.push(Access::Write(0x118, 1_u64.to_le_bytes().to_vec()));
        append_command(&mut accesses, command);
    }
    accesses
}

fn append_status(accesses: &mut Vec<Access>) {
    accesses.push(Access::Write(0x100, 0_u64.to_le_bytes().to_vec()));
    accesses.push(Access::Write(0x108, 0_u64.to_le_bytes().to_vec()));
    append_command(accesses, 2);
}

fn append_command(accesses: &mut Vec<Access>, command: u32) {
    accesses.push(Access::Write(0x168, command.to_le_bytes().to_vec()));
    accesses.push(Access::Read(0x16c, 4));
    for offset in (0x188..=0x1c8).step_by(8) {
        accesses.push(Access::Read(offset, 8));
    }
}

#[path = "cases.rs"]
mod cases;
