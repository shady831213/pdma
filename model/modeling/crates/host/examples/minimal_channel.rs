//! Single-channel copy and sticky-fault interrupt delivery on a software CPU.

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, InterruptHandler, MemoryAccess, MemoryAddress,
    MemoryAlignment, NoopInterruptObserver, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, Response, ResponseStatus};
use pdma_model_device::{Device, Fault, pipeline};
use pdma_model_host::{Channel, Irq, Model, RingConfig, Rings};
use std::{error::Error, sync::Arc};
use tokio::{sync::oneshot, task::JoinSet};

type HostChannel = Channel<Model<SharedMemoryAccess>, SharedMemoryAccess>;
type HostError = pdma_model_host::Error<pdma_model_device::Error>;

struct CompletionHandler {
    channel: HostChannel,
    completion: Option<oneshot::Sender<(Option<Response>, u32)>>,
}

impl InterruptHandler<Irq> for CompletionHandler {
    type Error = HostError;

    async fn handle(&mut self, _: &Irq) -> Result<(), HostError> {
        let fault = self.channel.fault().await?;
        let response = self.channel.response().await?;
        if fault != 0 {
            self.channel.set_irq_enabled(false).await?;
        }
        if let Some(completion) = self.completion.take() {
            let _receiver_closed = completion.send((response, fault));
        }
        Ok(())
    }
}

async fn run(fault_case: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(allocator.allocate(64, alignment)?.raw(), 2)?,
        response: RingConfig::new(
            if fault_case {
                0x8000
            } else {
                allocator.allocate(32, alignment)?.raw()
            },
            2,
        )?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    memory.write(source, alignment, b"dma-copy")?;
    let model = Model::new(Device::new(memory.clone()));
    let cpu = Arc::new(SoftwareCpu::new());
    let mut workers = JoinSet::new();
    let worker_model = model.clone();
    workers.spawn(async move {
        let executor = AsyncExecutor::new(worker_model);
        let mut stage = pipeline();
        let mut operation = Operation::new(());
        executor.run(&mut stage, &mut operation).await
    });
    let (completion, received) = oneshot::channel();
    let (handoff, handler_channel) = oneshot::channel();
    let software = cpu.run_main(async {
        let mut channel = Channel::setup(model.clone(), memory.clone(), rings).await?;
        channel
            .submit(Command {
                id: 7,
                source: source.raw(),
                destination: destination.raw(),
                length: 8,
            })
            .await?;
        handoff
            .send(channel)
            .map_err(|_| std::io::Error::other("interrupt assembly stopped"))?;
        Ok::<_, Box<dyn Error + Send + Sync>>(received.await?)
    });
    let assembly = async {
        let channel = handler_channel.await?;
        let registry = HandlerRegistry::with_handler(
            (),
            CompletionHandler {
                channel,
                completion: Some(completion),
            },
        )
        .freeze()
        .map_err(|_| std::io::Error::other("completion registry is empty"))?;
        Ok::<_, Box<dyn Error + Send + Sync>>(InterruptDispatcher::start(
            Arc::clone(&cpu),
            registry,
            model.clone(),
            Arc::new(NoopInterruptObserver),
        ))
    };
    let (completed, dispatcher) = tokio::join!(software, assembly);
    let dispatcher = dispatcher?;
    let summary = dispatcher.shutdown_and_join().await?;
    let (response, fault) = completed?;
    let result = workers
        .join_next()
        .await
        .ok_or_else(|| std::io::Error::other("device worker is missing"))??;
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"dma-copy");
    assert_eq!(summary.deliveries(), 1);
    assert!(!model.irq());
    if fault_case {
        assert_eq!(fault, Fault::ResponseWrite.code());
        assert_eq!(response, None);
        assert!(result.is_err());
        println!("fault: copied={copied:?}, sticky_fault={fault}, IRQ handled and masked");
    } else {
        assert_eq!(
            response,
            Some(Response {
                id: 7,
                status: ResponseStatus::Success
            })
        );
        assert_eq!(fault, 0);
        assert!(result.is_ok());
        println!("copy: copied={copied:?}, response={response:?}, IRQ handled and cleared");
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run(false).await?;
    run(true).await
}

#[test]
fn example_delivers_completion_and_fault_through_dispatcher()
-> Result<(), Box<dyn Error + Send + Sync>> {
    // Given: the example assembles real memory, hardware, executor, and software CPU.
    // When: its complete program runs both copy and fault scenarios.
    main()?;
    // Then: the program's response, payload, IRQ and delivery assertions all passed.
    Ok(())
}
