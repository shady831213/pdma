use super::{MmioDeviceBus, MmioDeviceTransport, read_u32, read_u64, write_u32};
use pdma_model_contract::abi::mmio::{IrqSourceStatus, MmioResult};
use pdma_model_contract::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId,
    IrqNotification, IrqSnapshot, IrqSource, IrqVector, QueueId,
};

enum Claim {
    Empty,
    Notification(IrqNotification),
}

pub(super) async fn snapshot<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
) -> Option<IrqSnapshot> {
    let _window = bus.irq_window.lock().await;
    write_u32(bus, 0x220, 8).await.ok()?;
    match result(bus).await? {
        MmioResult::Ok => {
            parse_notification(bus)
                .await
                .and_then(|notification| match notification {
                    IrqNotification::Line(snapshot) => Some(snapshot),
                    IrqNotification::Message(_) => None,
                })
        }
        MmioResult::NotExecuted
        | MmioResult::Empty
        | MmioResult::UnsupportedVersion
        | MmioResult::UnknownFeatureBits
        | MmioResult::StaleEpoch
        | MmioResult::InvalidQueue
        | MmioResult::InvalidIrqSource
        | MmioResult::IrqSourceActive
        | MmioResult::InvalidRingDepth
        | MmioResult::RingExtentOverflow
        | MmioResult::MisalignedRing
        | MmioResult::RingExtentMismatch
        | MmioResult::UnexpectedRingRecord
        | MmioResult::QueueDepthMismatch
        | MmioResult::EventDepthMismatch
        | MmioResult::CursorRegression
        | MmioResult::CursorUpdateRegression
        | MmioResult::CursorDistanceExceeded
        | MmioResult::CursorIncrementOverflow
        | MmioResult::DeviceEpochOverflow
        | MmioResult::UnknownStatusValue
        | MmioResult::DmaAddressExhausted
        | MmioResult::DmaStorageExhausted
        | MmioResult::DmaUnmapped
        | MmioResult::DmaCrossesRegion
        | MmioResult::DmaInvalidRegion
        | MmioResult::DmaOverlap
        | MmioResult::DmaStaleStage
        | MmioResult::InvalidTransaction
        | MmioResult::UnknownCommand
        | MmioResult::UnknownEncoding => None,
    }
}

pub(super) async fn wait_line<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
) -> Option<IrqSnapshot> {
    loop {
        if !bus.transport.wait_irq_notification_ready().await {
            return None;
        }
        if let Some(snapshot) = snapshot(bus).await {
            return Some(snapshot);
        }
    }
}

pub(super) async fn wait_notification<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
) -> Option<IrqNotification> {
    loop {
        if !bus.transport.wait_irq_notification_ready().await {
            return None;
        }
        let claim = {
            let _window = bus.irq_window.lock().await;
            claim_once(bus).await?
        };
        match claim {
            Claim::Empty => {}
            Claim::Notification(notification) => return Some(notification),
        }
    }
}

async fn claim_once<T: MmioDeviceTransport>(bus: &MmioDeviceBus<T>) -> Option<Claim> {
    write_u32(bus, 0x220, 9).await.ok()?;
    match result(bus).await? {
        MmioResult::Ok => parse_notification(bus).await.map(Claim::Notification),
        MmioResult::Empty => Some(Claim::Empty),
        _ => None,
    }
}

async fn result<T: MmioDeviceTransport>(bus: &MmioDeviceBus<T>) -> Option<MmioResult> {
    MmioResult::try_from(read_u32(bus, 0x224).await.ok()?).ok()
}

async fn parse_notification<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
) -> Option<IrqNotification> {
    let epoch = DeviceEpoch::new(read_u64(bus, 0x240).await.ok()?);
    let source_id = usize::try_from(read_u64(bus, 0x248).await.ok()?).ok()?;
    let source = match read_u32(bus, 0x250).await.ok()? {
        0 => IrqSource::Completion(CompletionRingId::new(source_id)),
        1 => IrqSource::Event(QueueId::new(source_id)),
        _ => return None,
    };
    IrqSourceStatus::try_from(read_u32(bus, 0x254).await.ok()?).ok()?;
    let route_kind = read_u32(bus, 0x258).await.ok()?;
    let notification_kind = read_u32(bus, 0x25c).await.ok()?;
    let vector = IrqVector::new(usize::try_from(read_u64(bus, 0x260).await.ok()?).ok()?);
    let message_id = usize::try_from(read_u64(bus, 0x268).await.ok()?).ok()?;
    read_u64(bus, 0x270).await.ok()?;
    let source = EpochIrqSource::new(epoch, source);
    match (notification_kind, route_kind) {
        (1, 0) if message_id == 0 => Some(IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(IrqLine::new(vector), source),
            true,
        ))),
        (2, 1) => Some(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(message_id),
            vector,
            source,
        ))),
        _ => None,
    }
}
