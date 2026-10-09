pub(in crate::harness) mod management;
/// Software-owned tagged recovery with two channels in one address space.
pub mod shared_space;
mod transfer;

use super::{Handler, HostVector, Ports, TestResult};
use crate::harness::{
    contexts::ATTRIBUTES,
    page_fault::{FaultReport, Space},
    runtime::AccessError,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, RegisterAccess};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, Response, ResponseStatus},
    memory::FunctionId,
};
use tokio::sync::{mpsc, oneshot};

pub(in crate::harness) struct Grant {
    pub channel: ChannelId,
    pub contexts: [ContextId; 4],
}

pub(in crate::harness) struct Recovery {
    pub sources: crate::harness::page_fault::platform::Sources,
    pub spaces: [Space; 3],
    pub reports: mpsc::Receiver<FaultReport>,
    pub grant: oneshot::Receiver<Grant>,
    pub finished: oneshot::Sender<()>,
    pub released: oneshot::Receiver<()>,
}

#[derive(Debug, Default)]
/// Software-observed platform faults and the original command's completion.
pub struct Outcome {
    pub(crate) responses: Vec<Response>,
    pub(crate) faults: Vec<FaultReport>,
    pub(crate) faulted: Vec<bool>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) closed_resume: Option<Result<(), AccessError>>,
}

pub(in crate::harness) async fn main<M: Send>(
    ports: Ports<'_, M>,
    recovery: Recovery,
) -> TestResult<Outcome> {
    let Recovery {
        sources,
        spaces: [lists, source, destination],
        mut reports,
        grant,
        finished,
        released,
    } = recovery;
    let Grant { channel, contexts } = grant.await?;
    let mut handler = sources
        .handler(FunctionId::new(1), &[channel])
        .ok_or("unknown granted source")?;
    let (mut driver, request, dst) =
        transfer::prepare(&ports, channel, contexts, (&lists, &source, &destination)).await?;
    let (wake, mut notifications) = mpsc::unbounded_channel();
    ports
        .interrupts
        .register(HostVector::Dma, Handler::Wake(wake))?;
    let mut outcome = Outcome::default();
    driver.submit(request).await?;
    for _ in 0..2 {
        let report = tokio::select! {
            report = reports.recv() => report.ok_or("platform fault reporter stopped")?,
            _ = notifications.recv() => return Err("page fault generated a DMA interrupt".into()),
        };
        if handler.resolve(&report) != Some(channel) {
            return Err("fault source is outside this grant".into());
        }
        if driver.response().await?.is_some() {
            return Err("suspended command published a response".into());
        }
        let faulted = loop {
            let faulted = driver.faulted().await?;
            if faulted {
                break faulted;
            }
            tokio::task::yield_now().await;
        };
        if report.identity.function != FunctionId::new(1) {
            return Err("fault reported for the wrong Function".into());
        }
        let space = [(ATTRIBUTES[2], &source), (ATTRIBUTES[3], &destination)]
            .into_iter()
            .find(|(attributes, _)| *attributes == report.identity.attributes)
            .map(|(_, space)| space)
            .ok_or("unknown fault address space")?;
        if !space.pages.repair(report.address) {
            return Err("reported page was not missing in its address space".into());
        }
        outcome.faults.push(report);
        outcome.faulted.push(faulted);
        driver.resume().await?;
    }
    notifications.recv().await.ok_or("DMA ISR stopped")?;
    let response = driver
        .response()
        .await?
        .ok_or("notification without response")?;
    if response.id != request.id || response.status != ResponseStatus::Success {
        return Err("original command failed to complete".into());
    }
    outcome.responses.push(response);
    if driver.response().await?.is_some() {
        return Err("duplicate command response".into());
    }
    outcome.bytes.resize(600, 0);
    destination
        .allocator
        .memory_access()
        .read(dst, MemoryAlignment::ONE, &mut outcome.bytes)?;
    handler.retire();
    reports.close();
    finished
        .send(())
        .map_err(|()| "PF stopped before release")?;
    released.await?;
    let address = ports.bus.addresses().dma
        + u32::try_from(
            channel.register_offset(1).ok_or("channel address")?
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + pdma_model_contract::dma::ChannelRegisters::RESUME.offset(),
        )?;
    outcome.closed_resume = Some(ports.bus.write(address, &1_u32.to_le_bytes()).await);
    Ok(outcome)
}
