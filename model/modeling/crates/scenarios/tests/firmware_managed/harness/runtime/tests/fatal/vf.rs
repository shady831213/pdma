use crate::harness::runtime::Host;
use hardware_interface_io::memory::MemoryAccessError;
use hardware_interface_io::memory::SharedMemoryAccess;
use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::dma::ChannelId;
use pdma_model_software::control::ControlIsr;
use tokio::sync::{mpsc, oneshot};

pub(super) struct Messages {
    pub grant: oneshot::Receiver<ChannelId>,
    pub ready: oneshot::Sender<()>,
    pub checks: mpsc::Receiver<oneshot::Sender<()>>,
    pub resume: oneshot::Receiver<()>,
    pub finished: oneshot::Sender<()>,
}

pub(super) async fn run(
    host: Host<SharedMemoryAccess<(), MemoryAccessError>>,
    allocator: SharedMemoryAllocator,
    mut messages: Messages,
) -> usize {
    let ((), summary) = host
        .runtime
        .run_main::<ControlIsr, _>(async |_| {
            let id = messages.grant.await.unwrap();
            super::copy(&host.bus, &allocator, id).await;
            messages.ready.send(()).unwrap();
            while let Some(confirm) = messages.checks.recv().await {
                assert!(matches!(
                    host.bus
                        .read(
                            u32::try_from(id.register_offset(1).unwrap()).unwrap(),
                            &mut [0; 8]
                        )
                        .await,
                    Err(super::AccessError::Closed)
                ));
                confirm.send(()).unwrap();
            }
            messages.resume.await.unwrap();
            super::copy(&host.bus, &allocator, id).await;
            messages.finished.send(()).unwrap();
        })
        .await
        .unwrap();
    summary.deliveries()
}
