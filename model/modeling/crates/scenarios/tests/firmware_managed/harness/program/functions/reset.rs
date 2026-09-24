//! Software programs coordinating an active channel reset with peer DMA.
use super::super::{Ports, TestResult, buffers};
use super::Grant;
use pdma_model_contract::dma::Response;
use tokio::sync::oneshot;

pub(in crate::harness) struct ResetSignals {
    pub(crate) started: oneshot::Sender<()>,
    pub(crate) submitted: oneshot::Receiver<()>,
    pub(crate) completed: oneshot::Receiver<()>,
    pub(crate) reset: oneshot::Sender<()>,
    pub(crate) reset_done: oneshot::Receiver<()>,
}

pub(in crate::harness) struct PeerSignals {
    pub(crate) started: oneshot::Receiver<()>,
    pub(crate) submitted: oneshot::Sender<()>,
    pub(crate) completed: oneshot::Sender<()>,
}

#[derive(Debug)]
/// Old-memory snapshots and fresh work observed around a Function's channel reset.
pub struct Outcome {
    pub(crate) before_reset: usize,
    pub(crate) at_reset: Vec<u8>,
    pub(crate) after_peer: Vec<u8>,
    pub(crate) fresh: Response,
    pub(crate) fresh_bytes: Vec<u8>,
}

pub(in crate::harness) async fn resetting(
    ports: Ports<'_>,
    grant: Grant,
    signals: ResetSignals,
) -> TestResult<Outcome> {
    let id = grant.channel.await?;
    let mut old = buffers::Transfer::new(&ports, id, &vec![0x6d; 65_536]).await?;
    old.start(100).await?;
    let before_reset = old.wait_started(&ports.memory).await?;
    signals.started.send(()).map_err(|()| "peer stopped")?;
    signals.submitted.await?;
    let destination = old.retire();
    signals
        .reset
        .send(())
        .map_err(|()| "PF stopped before reset")?;
    signals.reset_done.await?;
    let at_reset = buffers::read(&ports.memory, destination, 65_536)?;
    signals.completed.await?;
    let after_peer = buffers::read(&ports.memory, destination, 65_536)?;
    let mut fresh = buffers::Transfer::new(&ports, id, b"after-function-channel-reset").await?;
    fresh.start(101).await?;
    let response = fresh.completed().await?;
    let fresh_bytes = fresh.bytes(&ports.memory)?;
    drop(fresh);
    grant
        .finished
        .send(())
        .map_err(|()| "PF stopped before release")?;
    Ok(Outcome {
        before_reset,
        at_reset,
        after_peer,
        fresh: response,
        fresh_bytes,
    })
}

pub(in crate::harness) async fn peer(
    ports: Ports<'_>,
    grant: Grant,
    signals: PeerSignals,
) -> TestResult<(Response, Vec<u8>)> {
    let id = grant.channel.await?;
    let mut transfer = buffers::Transfer::new(&ports, id, b"peer-function-copy").await?;
    signals.started.await?;
    transfer.start(200).await?;
    signals
        .submitted
        .send(())
        .map_err(|()| "reset program stopped")?;
    let response = transfer.completed().await?;
    let bytes = transfer.bytes(&ports.memory)?;
    signals
        .completed
        .send(())
        .map_err(|()| "reset program stopped")?;
    drop(transfer);
    grant
        .finished
        .send(())
        .map_err(|()| "PF stopped before release")?;
    Ok((response, bytes))
}
