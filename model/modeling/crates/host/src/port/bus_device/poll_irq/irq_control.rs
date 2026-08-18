use super::{AsyncPdmaBus, BusDevicePort, EpochIrqSource, IrqRoute};
use crate::ModelError;
use crate::ModelResult;

pub(in super::super) async fn configure<B>(
    port: &BusDevicePort<B>,
    source: EpochIrqSource,
    route: IrqRoute,
) -> ModelResult<()>
where
    B: AsyncPdmaBus,
{
    validate_source(port, source)?;
    port.poll_model(port.bus.configure_irq_route(source, route))
        .await
}

macro_rules! irq_control {
    ($name:ident, $method:ident) => {
        pub(in super::super) async fn $name<B>(
            port: &BusDevicePort<B>,
            source: EpochIrqSource,
        ) -> ModelResult<()>
        where
            B: AsyncPdmaBus,
        {
            validate_source(port, source)?;
            port.poll_model(port.bus.$method(source)).await
        }
    };
}

irq_control!(enable, enable_irq_source);
irq_control!(disable, disable_irq_source);
irq_control!(mask, mask_irq_source);
irq_control!(unmask, unmask_irq_source);
irq_control!(ack, ack_irq_source);

fn validate_source<B>(port: &BusDevicePort<B>, source: EpochIrqSource) -> ModelResult<()> {
    port.with_state(|state| {
        let queue = state
            .queue
            .ok_or_else(|| ModelError::UnknownIrqSource(source.source()))?;
        if source.epoch() != queue.handle.epoch() {
            return Err(ModelError::StaleDeviceEpoch {
                expected: queue.handle.epoch(),
                actual: source.epoch(),
            });
        }
        if source != queue.handle.completion_irq_source()
            && source != queue.handle.event_irq_source()
        {
            return Err(ModelError::UnknownIrqSource(source.source()));
        }
        Ok(())
    })?
}
