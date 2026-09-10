//! Case-owned expectations shared by linear and scatter/gather transfers.

mod faults;
mod linear;
mod scatter_gather;

use crate::{
    harness::{self, FaultInjection, TestResult},
    software,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{ChannelFaultCode, Response, ResponseStatus, Segment};

async fn copy(response_fault: bool, input: software::Input<'_>) -> TestResult<Vec<Segment>> {
    let injection = if response_fault {
        FaultInjection::ResponseWrite
    } else {
        FaultInjection::None
    };
    let report = harness::run(injection, input).await?;
    let software::Outcome {
        response,
        fault,
        destination,
    } = report.program?;
    let result = report.worker.ok_or("device worker is missing")?;
    let mut copied = Vec::with_capacity(input.data.len());
    for segment in &destination {
        if segment.length == 0 {
            continue;
        }
        let address = MemoryAddress::new(segment.address);
        let region = report
            .storage
            .regions()
            .into_iter()
            .find(|region| region.base() == address)
            .ok_or_else(|| std::io::Error::other("destination allocation is missing"))?;
        assert_eq!(region.alignment(), MemoryAlignment::ONE);
        let mut bytes = vec![0; usize::try_from(segment.length)?];
        report
            .storage
            .read(address, MemoryAlignment::ONE, &mut bytes)?;
        copied.extend(bytes);
    }
    assert_eq!(copied, input.data);
    assert_eq!(report.summary.deliveries(), 1);
    assert!(!report.irq);
    if response_fault {
        assert_eq!(fault, ChannelFaultCode::ResponseWrite);
        assert_eq!(response, None);
        assert!(result.is_err());
        println!("fault: copied={copied:?}, sticky_fault={fault:?}, IRQ handled and masked");
    } else {
        assert_eq!(
            response,
            Some(Response {
                id: 7,
                status: ResponseStatus::Success,
                bytes_completed: u64::try_from(input.data.len())?
            })
        );
        assert_eq!(fault, ChannelFaultCode::None);
        assert!(result.is_ok());
        println!("copy: copied={copied:?}, response={response:?}, IRQ handled and cleared");
    }
    Ok(destination)
}
