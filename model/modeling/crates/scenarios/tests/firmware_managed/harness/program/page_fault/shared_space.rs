mod setup;
mod work;

use super::{Ports, TestResult};
use crate::harness::{
    page_fault::{FaultReport, Space, platform::Sources, shared_space::Input},
    program::{
        Handler, HostVector,
        buffers::{self, Driver},
    },
};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{
    control::ControlResponseStatus,
    dma::{ChannelId, Command, ContextId, Event},
    memory::{ContextAttributes, FunctionId},
};
use pdma_model_software::control::InterruptEvent;
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug)]
/// Management operation performed after retiring the original fault handler.
pub enum Lifecycle {
    /// Reset while retaining the grant.
    Reset,
    /// Release and acquire a fresh grant.
    Release,
}

pub(in crate::harness) struct Recovery {
    pub payload: Space,
    pub sources: Sources,
    pub foreign_sources: Sources,
    pub reports: mpsc::Receiver<FaultReport>,
}

/// Observable report correlation, completion and handler-retirement results.
#[derive(Debug)]
pub struct Outcome {
    pub(crate) faults: Vec<FaultReport>,
    pub(crate) completions: Vec<u64>,
    pub(crate) bytes: Vec<Vec<u8>>,
    pub(crate) peer_stayed_parked: bool,
    pub(crate) stale_rejected: bool,
    pub(crate) foreign_rejected: bool,
}

pub(in crate::harness) async fn main<M: Send>(
    ports: Ports<'_, M>,
    mut recovery: Recovery,
    input: &Input,
) -> TestResult<Outcome> {
    let mut session = setup::session(&ports, input.attributes).await?;
    let work::Parked {
        mut work,
        faults: [first, second],
        mut handler,
    } = work::pair(&ports, &mut recovery, (&session, input)).await?;
    let Recovery {
        payload,
        sources,
        foreign_sources,
        mut reports,
    } = recovery;
    let function = FunctionId::new(0);
    let memory = payload.allocator.memory_access();
    let selected = handler.resolve(&first).ok_or("unknown first source")?;
    let peer = handler.resolve(&second).ok_or("unknown second source")?;
    if selected == peer {
        return Err("same-space faults attributed to one channel".into());
    }

    if !payload.pages.repair(first.address) {
        return Err("shared page was present".into());
    }
    let task = work
        .iter_mut()
        .find(|task| task.channel == selected)
        .ok_or("source outside grant")?;
    let request = task.request;
    task.driver.resume().await?;
    let first_response = setup::completed(&mut task.driver, (&ports.memory, task.fence)).await?;
    let peer_task = work
        .iter_mut()
        .find(|task| task.channel == peer)
        .ok_or("peer outside grant")?;
    let peer_stayed_parked =
        peer_task.driver.faulted().await? && peer_task.driver.event().await?.is_none();
    peer_task.driver.resume().await?;
    let second_response =
        setup::completed(&mut peer_task.driver, (&ports.memory, peer_task.fence)).await?;
    let foreign = foreign_sources
        .handler(function, &session.channels)
        .ok_or("foreign catalog")?;
    let foreign_rejected = foreign.resolve(&first).is_none();
    handler.retire();
    if reports.try_recv().is_ok() {
        return Err("undrained fault report before lifecycle".into());
    }
    let new_channel = session.recycle(input.lifecycle, selected).await?;
    let mut next_handler = sources
        .handler(function, &[new_channel])
        .ok_or("unknown new source")?;
    let mut next_driver = setup::driver(&ports, new_channel, session.ring_context).await?;
    payload.pages.evict(first.address);
    let Command::Copy(copy) = request else {
        return Err("expected Copy".into());
    };
    next_driver
        .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: input.ids[2],
            ..copy
        }))
        .await?;
    let fence = setup::fence(&ports, session.ring_context, input.ids[2])?;
    next_driver.submit(fence.command(input.ids[2])).await?;
    let fresh = reports.recv().await.ok_or("missing fresh fault")?;
    setup::parked(&mut next_driver).await?;
    let stale_rejected = handler.resolve(&first).is_none()
        && next_driver.faulted().await?
        && next_driver.event().await?.is_none();
    if next_handler.resolve(&fresh) != Some(new_channel) {
        return Err("new fault source mismatch".into());
    }
    if !payload.pages.repair(fresh.address) {
        return Err("fresh page was present".into());
    }
    next_driver.resume().await?;
    let last = setup::completed(&mut next_driver, (&ports.memory, fence)).await?;
    next_handler.retire();
    reports.close();
    session
        .manager
        .release_channel(function, new_channel)
        .await?;
    session.manager.release_channel(function, peer).await?;
    let bytes = work
        .iter()
        .map(|task| buffers::read(&memory, task.destination, input.payload.len()))
        .collect::<TestResult<_>>()?;
    Ok(Outcome {
        faults: vec![first, second],
        completions: vec![first_response, second_response, last],
        bytes,
        peer_stayed_parked,
        stale_rejected,
        foreign_rejected,
    })
}
