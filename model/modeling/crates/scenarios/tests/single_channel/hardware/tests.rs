//! Assembly must keep serving after the software observes its first completion.

use crate::{harness::TestResult, software};
use hardware_interface_pipeline::{AsyncExecutor, HardwareDriver};
use hardware_interface_runtime::{
    InterruptRuntime, MemoryAddress, SharedMemory, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_host::runtime::Model;
use std::sync::Arc;
use tokio::task::JoinSet;

#[tokio::test(flavor = "current_thread")]
async fn assembled_hardware_remains_running_after_a_copy() -> TestResult {
    // Given: the public hardware assembly and a complete software copy program.
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(storage, MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (device, mut hardware) = super::assemble(memory.clone())?;
    let model = Model::new(device);
    let executor = AsyncExecutor::new(model.clone());
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    let mut workers = JoinSet::new();
    workers.spawn(async move {
        executor
            .drive(&mut hardware, &mut HardwareDriver::new())
            .await
    });
    // When: software completes one command within a finite scheduling budget.
    let execution = runtime.run_main(async |interrupts| {
        software::main(
            software::Ports {
                bus: model,
                memory,
                allocator,
                interrupts,
            },
            software::Input::linear(b"persistent-copy"),
        )
        .await
    });
    let result: TestResult<_> = tokio::select! {
        biased;
        result = execution => result.map_err(Into::into),
        () = async { for _ in 0..10_000 { tokio::task::yield_now().await; } } => {
            Err("copy exceeded its scheduling budget".into())
        }
    };
    tokio::task::yield_now().await;
    let premature = workers.try_join_next();
    workers.abort_all();
    let cancellation = workers.join_next().await;
    // Then: copy completion leaves hardware alive, and teardown joins its cancellation.
    let (outcome, summary) = result?;
    assert!(outcome?.response.is_some());
    assert_eq!(summary.deliveries(), 1);
    assert!(
        premature.is_none(),
        "hardware stopped after one command: {premature:?}"
    );
    assert!(matches!(cancellation, Some(Err(error)) if error.is_cancelled()));
    Ok(())
}
