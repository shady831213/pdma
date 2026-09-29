mod transfer;

use super::{Handler, HostVector, Ports, TestResult, buffers};
use crate::harness::{contexts::ATTRIBUTES, page_fault::Space};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{
    control::ControlResponseStatus,
    dma::{Command, ContextId, PageFaultInfo, PageFaultKindsAccess, Response, ResponseInfoValues},
    memory::{ContextAttributes, FunctionId},
};
use pdma_model_host::firmware;
use pdma_model_software::control::InterruptEvent;
use std::time::Duration;

#[derive(Debug, Default)]
/// Software-observed completions, repair identities and released resources.
pub struct Outcome {
    pub(crate) responses: Vec<Response>,
    pub(crate) repairs: Vec<(ContextAttributes, u64)>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) released: bool,
}

pub(in crate::harness) async fn main(
    ports: Ports<'_>,
    [lists, source, destination]: [Space; 3],
) -> TestResult<Outcome> {
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    let control_irq = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(control_irq.handler()))?;
    let mut management =
        buffers::management_with_context(&ports, control_irq, ATTRIBUTES[1]).await?;
    let function = FunctionId::new(0);
    let mut contexts = Vec::new();
    for attributes in ATTRIBUTES {
        management
            .submit_create_context(function, attributes)
            .await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err("context creation failed".into());
        }
        contexts.push(ContextId::new(response.context));
    }
    let [list_id, ring_id, source_id, destination_id]: [ContextId; 4] =
        contexts.try_into().map_err(|_| "context count")?;
    let channel = management.acquire_channel(function).await?;
    let (mut driver, mut request, dst) = transfer::prepare(
        &ports,
        channel,
        [list_id, ring_id, source_id, destination_id],
        (&lists, &source, &destination),
    )
    .await?;
    let (wake, mut notifications) = tokio::sync::mpsc::unbounded_channel();
    ports
        .interrupts
        .register(HostVector::Dma, Handler::Wake(wake))?;
    let mut outcome = Outcome::default();
    loop {
        driver.submit(request).await?;
        notifications.recv().await.ok_or("DMA ISR stopped")?;
        let response = driver
            .response()
            .await?
            .ok_or("notification without response")?;
        if response.id != request.id {
            return Err("unknown completion".into());
        }
        outcome.responses.push(response);
        match response.info {
            ResponseInfoValues::Success(_) => break,
            ResponseInfoValues::PageFault(fault) => {
                let (selector, attributes) = repair_payload(
                    &fault,
                    &request,
                    [
                        (source_id, &source, ATTRIBUTES[2]),
                        (destination_id, &destination, ATTRIBUTES[3]),
                    ],
                )?;
                outcome.repairs.push((attributes, fault.fault_address));
                management
                    .submit_drain_context(function, ContextId::new(selector))
                    .await?;
                if management.next_response().await?.status != ControlResponseStatus::Success {
                    return Err("faulted attempt did not retire".into());
                }
                request = transfer::continuation(request, fault)?;
            }
            ResponseInfoValues::InvalidLength(_)
            | ResponseInfoValues::ReadError(_)
            | ResponseInfoValues::WriteError(_)
            | ResponseInfoValues::LengthMismatch(_)
            | ResponseInfoValues::ListReadError(_) => return Err("unrecoverable DMA error".into()),
        }
    }
    outcome.bytes.resize(600, 0);
    destination
        .allocator
        .memory_access()
        .read(dst, MemoryAlignment::ONE, &mut outcome.bytes)?;
    management.release_channel(function, channel).await?;
    for id in [list_id, ring_id, source_id, destination_id] {
        management.submit_destroy_context(function, id).await?;
        if management.next_response().await?.status != ControlResponseStatus::Success {
            return Err("context destroy failed".into());
        }
    }
    outcome.released = true;
    Ok(outcome)
}

fn repair_payload(
    fault: &PageFaultInfo,
    request: &Command,
    bindings: [(ContextId, &Space, ContextAttributes); 2],
) -> TestResult<(u32, ContextAttributes)> {
    let selector = match fault.kinds.access {
        PageFaultKindsAccess::SourceRead => request.source.context,
        PageFaultKindsAccess::DestinationWrite => request.destination.context,
    };
    let (_, space, attributes) = bindings
        .into_iter()
        .find(|(id, _, _)| id.raw() == selector)
        .ok_or("unknown payload context")?;
    if !space.pages.repair(fault.fault_address.into()) {
        return Err("page not missing in retained address space".into());
    }
    Ok((selector, attributes))
}
