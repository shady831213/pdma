use super::{Spaces, TestResult, Transfer};
use crate::harness::runtime::HostBus;
use hw_model_io::memory::MemoryAccessError;
use hw_model_io::memory::SharedMemoryAccess;
use pdma_model_contract::control::ControlResponseStatus;
use pdma_model_contract::dma::{ContextId, Event};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Management;

#[derive(Debug)]
#[doc = "Observed peer progress and target retirement during one maintenance request."]
pub struct MixedOutcome {
    pub(crate) partial: usize,
    pub(crate) completions: Vec<Result<u64, Event>>,
    pub(crate) target: [u8; 4],
    pub(crate) peer: [u8; 4],
    pub(crate) status: ControlResponseStatus,
}

pub(super) async fn run(
    management: &mut Management<
        HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
        SharedMemoryAccess<(), MemoryAccessError>,
    >,
    transfer: &mut Transfer,
    (spaces, contexts): (&Spaces, [ContextId; 4]),
) -> TestResult<Vec<MixedOutcome>> {
    let function = FunctionId::new(0);
    let mut outcomes = Vec::new();
    for abort in [false, true] {
        let queued = transfer
            .mixed(spaces, (contexts, if abort { 200 } else { 100 }))
            .await?;
        if abort {
            management
                .submit_abort_context(function, contexts[2])
                .await?;
        } else {
            management
                .submit_drain_context(function, contexts[2])
                .await?;
        }
        let status = management.next_response().await?.status;
        let completions = vec![transfer.completed().await?];
        let (target, peer) = queued.bytes(spaces)?;
        outcomes.push(MixedOutcome {
            partial: queued.partial,
            completions,
            target,
            peer,
            status,
        });
    }
    Ok(outcomes)
}
