use super::{MmioDeviceBus, MmioDeviceTransport};
use pdma_model_contract::abi::mmio::{IrqSourceStatus, MmioByteCount, MmioResult};
use pdma_model_contract::{
    DeviceEpoch, EpochIrqSource, IrqRoute, IrqSource, ModelError, ModelResult,
};

pub(super) async fn source_command<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    source: EpochIrqSource,
    route: Option<IrqRoute>,
    command: u32,
) -> ModelResult<IrqSourceStatus> {
    let _window = bus.irq_window.lock().await;
    write_u64(bus, 0x200, source.epoch().raw()).await?;
    let (kind, id) = encode_source(source.source())?;
    write_u64(bus, 0x208, id).await?;
    write_u32(bus, 0x210, kind).await?;
    if let Some(route) = route {
        let (route_kind, vector) = encode_route(route)?;
        write_u32(bus, 0x214, route_kind).await?;
        write_u64(bus, 0x218, vector).await?;
    }
    write_u32(bus, 0x220, command).await?;
    let result = read_u32(bus, 0x224).await?;
    match MmioResult::try_from(result) {
        Ok(MmioResult::Ok) => read_u32(bus, 0x254).await.and_then(|bits| {
            IrqSourceStatus::try_from(bits).map_err(|_| ModelError::DmaSubmissionState)
        }),
        Ok(MmioResult::StaleEpoch) => {
            let expected = read_u64(bus, 0x228).await?;
            let actual = read_u64(bus, 0x230).await?;
            Err(ModelError::StaleDeviceEpoch {
                expected: DeviceEpoch::new(expected),
                actual: DeviceEpoch::new(actual),
            })
        }
        Ok(MmioResult::InvalidIrqSource) => Err(ModelError::UnknownIrqSource(source.source())),
        Ok(MmioResult::IrqSourceActive) => Err(ModelError::IrqSourceActive(source.source())),
        Ok(
            MmioResult::NotExecuted
            | MmioResult::Empty
            | MmioResult::UnsupportedVersion
            | MmioResult::UnknownFeatureBits
            | MmioResult::InvalidQueue
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
            | MmioResult::UnknownEncoding,
        )
        | Err(_) => Err(ModelError::DmaSubmissionState),
    }
}

fn encode_source(source: IrqSource) -> ModelResult<(u32, u64)> {
    match source {
        IrqSource::Completion(id) => u64::try_from(id.raw())
            .map(|raw| (0, raw))
            .map_err(|_| ModelError::DmaSubmissionState),
        IrqSource::Event(id) => u64::try_from(id.raw())
            .map(|raw| (1, raw))
            .map_err(|_| ModelError::DmaSubmissionState),
    }
}

fn encode_route(route: IrqRoute) -> ModelResult<(u32, u64)> {
    match route {
        IrqRoute::Line(line) => u64::try_from(line.vector().raw())
            .map(|raw| (0, raw))
            .map_err(|_| ModelError::DmaSubmissionState),
        IrqRoute::Message(vector) => u64::try_from(vector.raw())
            .map(|raw| (1, raw))
            .map_err(|_| ModelError::DmaSubmissionState),
    }
}

async fn write_u32<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    offset: u32,
    value: u32,
) -> ModelResult<()> {
    bus.transport
        .write_mmio(offset, &value.to_le_bytes())
        .await
        .map_err(|_| ModelError::DmaSubmissionState)
}

async fn write_u64<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    offset: u32,
    value: u64,
) -> ModelResult<()> {
    bus.transport
        .write_mmio(offset, &value.to_le_bytes())
        .await
        .map_err(|_| ModelError::DmaSubmissionState)
}

async fn read_u32<T: MmioDeviceTransport>(bus: &MmioDeviceBus<T>, offset: u32) -> ModelResult<u32> {
    let bytes = bus
        .transport
        .read_mmio(offset, MmioByteCount::from(4_u8))
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    <[u8; 4]>::try_from(bytes.as_slice())
        .map(u32::from_le_bytes)
        .map_err(|_| ModelError::DmaSubmissionState)
}

async fn read_u64<T: MmioDeviceTransport>(bus: &MmioDeviceBus<T>, offset: u32) -> ModelResult<u64> {
    let bytes = bus
        .transport
        .read_mmio(offset, MmioByteCount::from(8_u8))
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    <[u8; 8]>::try_from(bytes.as_slice())
        .map(u64::from_le_bytes)
        .map_err(|_| ModelError::DmaSubmissionState)
}
