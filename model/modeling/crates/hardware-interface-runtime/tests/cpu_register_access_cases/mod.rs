mod fixture;

use fixture::{Registers, poll_once};
use hardware_interface_runtime::{CpuAccessError, CpuContextError, RegisterAccess, SoftwareCpu};
use std::{error::Error, io};

#[tokio::test]
async fn read_and_write_succeed_when_owner_runs_main() {
    // Given: registers bound while no CPU is active.
    let cpu = SoftwareCpu::new();
    let endpoint = Registers::suspended();
    let registers = cpu.bind_registers(endpoint.clone());
    let mut response = [0; 4];
    // When: the owning main program writes then reads its registers.
    cpu.run_main(async {
        registers.write(0, &[1, 2, 3, 4]).await.expect("write");
        registers.read(0, &mut response).await.expect("read");
    })
    .await;
    // Then: exact bytes pass through the bound capability.
    assert_eq!(response, [1, 2, 3, 4]);
    assert_eq!(endpoint.counts(), (2, 4, 2));
}

#[tokio::test]
async fn read_and_write_succeed_when_owner_runs_handler() {
    // Given: a capability bound to the handler's CPU.
    let cpu = SoftwareCpu::new();
    let registers = cpu.bind_registers(Registers::default());
    let mut response = [0; 4];
    // When: the handler accesses its registers.
    cpu.run_handler_turn(async {
        registers.write(0, &[7; 4]).await.expect("write");
        registers.read(0, &mut response).await.expect("read");
    })
    .await;
    // Then: the handler receives the bytes it published.
    assert_eq!(response, [7; 4]);
}

#[tokio::test]
async fn accesses_reject_inactive_cpu_before_constructing_transport_future() {
    // Given: a bound capability outside its owning CPU.
    let cpu = SoftwareCpu::new();
    let endpoint = Registers::default();
    let registers = cpu.bind_registers(endpoint.clone());
    let mut response = [3; 4];
    // When: read and write are driven without a CPU.
    let read = registers.read(0, &mut response).await;
    let write = registers.write(0, &[7; 4]).await;
    // Then: neither transport invocation nor destination mutation occurred.
    assert!(matches!(
        read,
        Err(CpuAccessError::Cpu(CpuContextError::Inactive))
    ));
    assert!(matches!(
        write,
        Err(CpuAccessError::Cpu(CpuContextError::Inactive))
    ));
    assert_eq!(response, [3; 4]);
    assert_eq!(endpoint.counts(), (0, 0, 0));
}

#[tokio::test]
async fn accesses_reject_other_cpu_before_constructing_transport_future() {
    // Given: registers belong to one CPU while another runs.
    let cpu = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let endpoint = Registers::default();
    let registers = cpu.bind_registers(endpoint.clone());
    let mut response = [3; 4];
    // When: the other CPU reads and writes.
    let (read, write) = other
        .run_main(async {
            (
                registers.read(0, &mut response).await,
                registers.write(0, &[7; 4]).await,
            )
        })
        .await;
    // Then: both fail before touching the transport.
    assert!(matches!(
        read,
        Err(CpuAccessError::Cpu(CpuContextError::Mismatch))
    ));
    assert!(matches!(
        write,
        Err(CpuAccessError::Cpu(CpuContextError::Mismatch))
    ));
    assert_eq!(response, [3; 4]);
    assert_eq!(endpoint.counts(), (0, 0, 0));
}

#[tokio::test]
async fn clone_retains_explicit_owner_when_bound_from_another_cpu() {
    // Given: binding and cloning happen while a different CPU is active.
    let cpu = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let endpoint = Registers::default();
    let (original, registers) = other
        .run_main(async {
            let original = cpu.bind_registers(endpoint.clone());
            let cloned = original.clone();
            (original, cloned)
        })
        .await;
    // When: both CPUs try the cloned capability.
    let rejected = other.run_main(registers.write(0, &[7; 4])).await;
    let accepted = cpu.run_main(registers.write(0, &[8; 4])).await;
    let original_accepted = cpu.run_main(original.write(0, &[9; 4])).await;
    // Then: the explicitly selected owner alone performs an access.
    assert!(matches!(
        rejected,
        Err(CpuAccessError::Cpu(CpuContextError::Mismatch))
    ));
    assert!(accepted.is_ok());
    assert!(original_accepted.is_ok());
    assert_eq!(endpoint.counts(), (2, 2, 2));
}

#[tokio::test]
async fn suspended_read_rejects_other_cpu_before_repolling_transport() {
    // Given: a read suspended on its owner before filling the destination.
    let cpu = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let endpoint = Registers::suspended();
    let registers = cpu.bind_registers(endpoint.clone());
    let mut response = [3; 4];
    let result = {
        let read = registers.read(0, &mut response);
        tokio::pin!(read);
        assert!(cpu.run_main(poll_once(read.as_mut())).await.is_pending());
        // When: a different CPU resumes that same future.
        other.run_main(read).await
    };
    // Then: no second inner poll or read effect occurs.
    assert!(matches!(
        result,
        Err(CpuAccessError::Cpu(CpuContextError::Mismatch))
    ));
    assert_eq!(response, [3; 4]);
    assert_eq!(endpoint.counts(), (1, 1, 0));
}

#[tokio::test]
async fn suspended_write_rejects_other_cpu_before_repolling_transport() {
    // Given: a write suspended on its owner before committing bytes.
    let cpu = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let endpoint = Registers::suspended();
    let registers = cpu.bind_registers(endpoint.clone());
    let write = registers.write(0, &[7; 4]);
    tokio::pin!(write);
    assert!(cpu.run_main(poll_once(write.as_mut())).await.is_pending());
    // When: a different CPU resumes that same future.
    let result = other.run_main(write).await;
    // Then: no second inner poll or write effect occurs.
    assert!(matches!(
        result,
        Err(CpuAccessError::Cpu(CpuContextError::Mismatch))
    ));
    assert_eq!(endpoint.counts(), (1, 1, 0));
}

#[tokio::test]
async fn transport_errors_preserve_typed_cause_and_display() {
    // Given: a bound transport rejects unknown register offsets.
    let cpu = SoftwareCpu::new();
    let endpoint = Registers::default();
    let registers = cpu.bind_registers(endpoint.clone());
    let mut response = [3; 4];
    // When: read and write encounter the transport error on their owner.
    let errors = cpu
        .run_main(async {
            [
                registers.read(4, &mut response).await.expect_err("read"),
                registers.write(4, &[7; 4]).await.expect_err("write"),
            ]
        })
        .await;
    // Then: typed causes and their display remain available without effects.
    for error in errors {
        let CpuAccessError::Access(cause) = &error else {
            panic!("transport error")
        };
        assert_eq!(cause.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), cause.to_string());
        assert_eq!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::InvalidInput)
        );
    }
    assert_eq!(response, [3; 4]);
    assert_eq!(endpoint.counts(), (2, 2, 0));
}

#[tokio::test]
async fn cpu_error_preserves_typed_cause_and_display() {
    // Given: no active software CPU.
    let cpu = SoftwareCpu::new();
    let registers = cpu.bind_registers(Registers::default());
    // When: an access fails its CPU check.
    let error = registers.write(0, &[7; 4]).await.expect_err("inactive");
    // Then: the CPU cause can be recovered through the standard error chain.
    assert_eq!(error.to_string(), CpuContextError::Inactive.to_string());
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<CpuContextError>()),
        Some(&CpuContextError::Inactive)
    );
}
