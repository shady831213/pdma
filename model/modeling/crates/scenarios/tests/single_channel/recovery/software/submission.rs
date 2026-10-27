use super::*;
use hardware_interface_io::memory::MemoryAccessError;

pub(super) async fn initial<B>(
    channel: &mut Channel<B, SharedMemoryAccess<(), MemoryAccessError>>,
    scenario: Scenario,
    (request, fence): (CopyCommand, pdma_model_software::dma::CompletionWord),
) -> TestResult
where
    B: RegisterAccess + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    let CopyCommandDestinationValues::Linear(destination) = request.destination else {
        return Err("expected Linear destination".into());
    };
    match scenario {
        Scenario::Batch => {
            channel
                .set_irq_mask(ChannelIrqMaskValues::default().with_event_non_empty(true)?)
                .await?;
            for index in 0..3 {
                channel
                    .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
                        id: 40 + index,
                        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                            address: destination.address + index * request.copy_length,
                            offset: 0,
                        }),
                        ..request
                    }))
                    .await?;
            }
            channel.submit(fence.command(43)).await?;
            channel
                .submit(Command::Trap(TrapCommand {
                    header: CommandHeader {
                        opcode: Opcode::Trap,
                    },
                    id: 44,
                }))
                .await?;
        }
        Scenario::ChannelReset => {
            channel
                .set_irq_mask(
                    ChannelIrqMaskValues::default()
                        .with_event_non_empty(true)?
                        .with_trap(true)?,
                )
                .await?;
            channel
                .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
                    id: 41,
                    destination: CopyCommandDestinationValues::ScatterGather(
                        ScatterGatherEndpoint {
                            list_address: 0,
                            count: 0,
                            index: 0,
                            offset: 0,
                        },
                    ),
                    ..request
                }))
                .await?;
        }
    }
    Ok(())
}
