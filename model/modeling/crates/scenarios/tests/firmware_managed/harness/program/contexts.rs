use super::{Handler, HostVector, Ports, TestResult, buffers};
use hardware_interface_runtime::SharedMemoryAllocator;
use pdma_model_contract::control::{ControlResponse, ControlResponseStatus};
use pdma_model_contract::dma::{ContextId, Event};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::firmware;
use pdma_model_software::control::InterruptEvent;
use std::time::Duration;

mod data;
mod mixed;
use data::Transfer;
pub use mixed::MixedOutcome;

pub(in crate::harness) struct Spaces {
    pub lists: SharedMemoryAllocator,
    pub source: SharedMemoryAllocator,
    pub destination: SharedMemoryAllocator,
}

#[derive(Debug)]
#[doc = "Software-observed context lifecycle and memory outcomes."]
pub struct Outcome {
    pub(crate) completions: Vec<Result<u64, Event>>,
    pub(crate) management: Vec<ControlResponse>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) same_address: bool,
    pub(crate) mixed: Vec<MixedOutcome>,
}

pub(in crate::harness) async fn main(
    ports: Ports<'_>,
    spaces: Spaces,
    failure: bool,
) -> TestResult<Outcome> {
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    let interrupt = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(interrupt.handler()))?;
    let mut management = buffers::management_with_context(
        &ports,
        interrupt,
        crate::harness::contexts::ATTRIBUTES[1],
    )
    .await?;
    let function = FunctionId::new(0);
    let mut ids = Vec::new();
    let mut controls = Vec::new();
    for attributes in crate::harness::contexts::ATTRIBUTES {
        management
            .submit_create_context(function, attributes)
            .await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err(format!("create context: {:?}", response.status).into());
        }
        ids.push(ContextId::new(response.context));
        controls.push(response);
    }
    let contexts: [ContextId; 4] = ids.try_into().map_err(|_| "context count")?;
    let channel = management.acquire_channel(function).await?;
    let mut transfer = Transfer::new(&ports, (&spaces, channel), contexts).await?;
    let mut completions = Vec::new();
    transfer.submit(failure).await?;
    if failure {
        completions.push(transfer.completed().await?);
        management
            .submit_drain_context(function, contexts[2])
            .await?;
        controls.push(management.next_response().await?);
        management.reset_channel(function, channel).await?;
        transfer = Transfer::new(&ports, (&spaces, channel), contexts).await?;
        transfer.submit(false).await?;
    }
    completions.push(transfer.completed().await?);
    let mixed = if failure {
        Vec::new()
    } else {
        mixed::run(&mut management, &mut transfer, (&spaces, contexts)).await?
    };
    management
        .submit_drain_context(function, contexts[2])
        .await?;
    controls.push(management.next_response().await?);
    management
        .submit_abort_context(function, contexts[3])
        .await?;
    controls.push(management.next_response().await?);
    management
        .submit_destroy_context(function, contexts[1])
        .await?;
    controls.push(management.next_response().await?);
    let bytes = transfer.bytes(&spaces.destination.memory_access())?;
    let same_address = transfer.same_address;
    management.release_channel(function, channel).await?;
    for context in contexts {
        management.submit_destroy_context(function, context).await?;
        controls.push(management.next_response().await?);
    }
    management
        .submit_create_context(function, crate::harness::contexts::ATTRIBUTES[2])
        .await?;
    controls.push(management.next_response().await?);
    Ok(Outcome {
        completions,
        management: controls,
        bytes,
        same_address,
        mixed,
    })
}
