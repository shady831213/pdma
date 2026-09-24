//! Host-owned concurrent management and real DMA lifecycle exercise.

use super::buffers;

use super::{Handler, HostVector, Ports, TestResult};
use crate::harness::runtime::{AccessError, HostBus};
use buffers::Transfer;
use hardware_interface_runtime::{RegisterAccess, SharedMemoryAccess};
use pdma_model_contract::control::ControlResponse;
use pdma_model_contract::dma::{ChannelId, Response};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::{Management, firmware};
use pdma_model_software::control::InterruptEvent;
use std::{io, time::Duration};

#[derive(Debug)]
/// Correlated completions, permission probes and memory snapshots for case assertions.
pub struct Outcome {
    pub(crate) granted: [u32; 4],
    pub(crate) management: Vec<(u64, ControlResponse)>,
    pub(crate) partial_bytes: usize,
    pub(crate) drained_bytes: usize,
    pub(crate) old_destination: [Vec<u8>; 2],
    pub(crate) peer: Response,
    pub(crate) fresh: Response,
    pub(crate) peer_bytes: Vec<u8>,
    pub(crate) fresh_bytes: Vec<u8>,
    pub(crate) unallocated_access: Result<(), AccessError>,
    pub(crate) released_access: Result<(), AccessError>,
    pub(crate) peer_access: Result<(), AccessError>,
    pub(crate) final_status: [u32; 3],
}

pub(in crate::harness) async fn main(ports: Ports<'_>) -> TestResult<Outcome> {
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
    let mut management = buffers::management(&ports, interrupt).await?;
    let first = management.submit_acquire(FunctionId::new(0)).await?.raw();
    let second = management.submit_acquire(FunctionId::new(0)).await?.raw();
    let a = management.next_response().await?;
    let b = management.next_response().await?;
    let mut correlations = vec![(first, a), (second, b)];
    let a_id = ChannelId::new(usize::try_from(a.channel)?);
    let b_id = ChannelId::new(usize::try_from(b.channel)?);
    let unallocated_access = probe(&ports, ChannelId::new(2)).await?;
    let mut old = Transfer::new(&ports, a_id, &vec![0x6d; 65_536]).await?;
    let mut peer = Transfer::new(&ports, b_id, b"peer-copy-during-release").await?;
    old.start(101).await?;
    let partial_bytes = old.wait_started(&ports.memory).await?;
    peer.start(202).await?;
    let old_destination = old.retire();
    let acquire = management.submit_acquire(FunctionId::new(0)).await?.raw();
    let release = management
        .submit_release(FunctionId::new(0), a_id)
        .await?
        .raw();
    let mut release_done = None;
    let mut grant = None;
    let mut peer_response = None;
    let mut released_access = None;
    while release_done.is_none() || grant.is_none() || peer_response.is_none() {
        if let Some(response) = management.try_response().await? {
            if response.id == release {
                correlations.push((release, response));
                release_done = Some(buffers::read(&ports.memory, old_destination, 65_536)?);
                released_access = Some(probe(&ports, a_id).await?);
            } else if response.id == acquire {
                correlations.push((acquire, response));
                grant = Some(response);
            } else {
                return Err(io::Error::other("uncorrelated lifecycle response").into());
            }
        }
        if peer_response.is_none() {
            peer_response = peer.driver.response().await?;
        }
        tokio::task::yield_now().await;
    }
    let drained = release_done.ok_or("release did not complete")?;
    let c = grant.ok_or("third channel was not granted")?;
    let peer_access = probe(&ports, b_id).await?;
    let request = management.submit_acquire(FunctionId::new(0)).await?.raw();
    let regrant = management.next_response().await?;
    correlations.push((request, regrant));
    let mut fresh = Transfer::new(
        &ports,
        ChannelId::new(usize::try_from(regrant.channel)?),
        b"fresh-driver-after-drain",
    )
    .await?;
    fresh.start(303).await?;
    let fresh_response = fresh.completed().await?;
    let peer_bytes = peer.bytes(&ports.memory)?;
    let fresh_bytes = fresh.bytes(&ports.memory)?;
    drop(peer);
    drop(fresh);
    correlations
        .extend(release_all(&mut management, [b.channel, c.channel, regrant.channel]).await?);
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    let old_after = buffers::read(&ports.memory, old_destination, 65_536)?;
    let final_status = statuses(&ports.bus).await?;
    Ok(Outcome {
        granted: [a.channel, b.channel, c.channel, regrant.channel],
        management: correlations,
        partial_bytes,
        drained_bytes: buffers::copied_prefix(&drained),
        old_destination: [drained, old_after],
        peer: peer_response.ok_or("peer did not complete")?,
        fresh: fresh_response,
        peer_bytes,
        fresh_bytes,
        unallocated_access,
        released_access: released_access.ok_or("missing release access probe")?,
        peer_access,
        final_status,
    })
}

async fn probe(ports: &Ports<'_>, id: ChannelId) -> TestResult<Result<(), AccessError>> {
    let address = super::channel_address(&ports.bus, id).await?;
    Ok(ports.bus.read(address, &mut [0; 8]).await)
}

async fn release_all(
    management: &mut Management<HostBus<SharedMemoryAccess>, SharedMemoryAccess>,
    channels: [u32; 3],
) -> TestResult<Vec<(u64, ControlResponse)>> {
    let mut requests = Vec::new();
    for channel in channels {
        requests.push(
            management
                .submit_release(
                    FunctionId::new(0),
                    ChannelId::new(usize::try_from(channel)?),
                )
                .await?
                .raw(),
        );
    }
    let mut responses = Vec::new();
    for _ in channels {
        let response = management.next_response().await?;
        let index = requests
            .iter()
            .position(|id| *id == response.id)
            .ok_or("uncorrelated final release")?;
        responses.push((requests.remove(index), response));
    }
    Ok(responses)
}

async fn statuses(bus: &HostBus<SharedMemoryAccess>) -> TestResult<[u32; 3]> {
    let mut final_status = [0; 3];
    for (index, status) in final_status.iter_mut().enumerate() {
        let mut bytes = [0; 4];
        bus.read(
            bus.addresses()
                .window_status(ChannelId::new(index))
                .ok_or("status address")?,
            &mut bytes,
        )
        .await?;
        *status = u32::from_le_bytes(bytes);
    }
    Ok(final_status)
}
