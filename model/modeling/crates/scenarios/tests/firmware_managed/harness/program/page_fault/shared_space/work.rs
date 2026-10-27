use pdma_model_contract::dma::{CopyCommandDestinationValues, LinearEndpoint};

use super::*;
use crate::harness::page_fault::platform::FaultHandler;
use hardware_interface_io::memory::MemoryAddress;

pub(super) struct Work<M> {
    pub channel: ChannelId,
    pub driver: Driver<M>,
    pub request: Command,
    pub destination: MemoryAddress,
    pub fence: pdma_model_software::dma::CompletionWord,
}

pub(super) struct Parked<M> {
    pub work: Vec<Work<M>>,
    pub faults: [FaultReport; 2],
    pub handler: FaultHandler,
}

pub(super) async fn pair<M: Send>(
    ports: &Ports<'_, M>,
    recovery: &mut Recovery,
    (session, input): (&setup::Session<M>, &Input),
) -> TestResult<Parked<M>> {
    let handler = recovery
        .sources
        .handler(FunctionId::new(0), &session.channels)
        .ok_or("unknown source")?;
    let (request, destinations) =
        setup::requests(&recovery.payload, session.payload_context, &input.payload)?;
    let mut work = Vec::new();
    for ((channel, destination), id) in session
        .channels
        .into_iter()
        .zip(destinations)
        .zip(input.ids)
    {
        work.push(Work {
            channel,
            driver: setup::driver(ports, channel, session.ring_context).await?,
            request: Command::Copy(pdma_model_contract::dma::CopyCommand {
                id,
                destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                    address: destination.raw(),
                    offset: 0,
                }),
                destination_context: session.payload_context.raw(),
                ..match request {
                    pdma_model_contract::dma::Command::Copy(copy) => copy,
                    _ => return Err("expected Copy".into()),
                }
            }),
            fence: setup::fence(ports, session.ring_context, id)?,
            destination,
        });
    }
    let mut faults = Vec::new();
    for task in work.iter_mut().rev() {
        task.driver.submit(task.request).await?;
        task.driver
            .submit(task.fence.command(task.request.id()))
            .await?;
        let report = recovery
            .reports
            .recv()
            .await
            .ok_or("missing channel fault")?;
        if handler.resolve(&report) != Some(task.channel) {
            return Err("fault tag identifies the wrong originating channel".into());
        }
        faults.push(report);
    }
    for task in &mut work {
        setup::parked(&mut task.driver).await?;
    }
    Ok(Parked {
        work,
        faults: faults.try_into().map_err(|_| "fault count")?,
        handler,
    })
}
