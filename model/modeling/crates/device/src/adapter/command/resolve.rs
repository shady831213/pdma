use crate::memory::MappedDmaRegion;
use crate::memory::PdmaMemoryMappings;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiBounds, AbiEventAccess, AbiEventStage, AbiFault, AbiFaultSite, AbiOperandVisit,
    AbiStrideOffset, DmaAddress,
};
use pdma_model_contract::semantic::DmaRangeStart;

pub(super) fn direct(
    memory: &PdmaMemoryMappings,
    address: DmaAddress,
    length: u64,
    visit: AbiOperandVisit,
) -> ModelResult<Result<DmaRangeStart, AbiFault>> {
    let site = site(visit);
    let Ok(end) = address.checked_add(length) else {
        return Ok(Err(AbiFault::range_overflow_direct(address, length, site)));
    };
    resolve_representable(memory, address, end, length, visit)
}

pub(super) fn strided(
    memory: &PdmaMemoryMappings,
    base: DmaAddress,
    row: u64,
    stride: u64,
    length: u64,
    visit: AbiOperandVisit,
) -> ModelResult<Result<DmaRangeStart, AbiFault>> {
    let start = match row
        .checked_mul(stride)
        .and_then(|offset| base.raw().checked_add(offset))
    {
        Some(start) => DmaAddress::new(start),
        None => {
            return Ok(Err(AbiFault::range_overflow_strided(
                base,
                AbiStrideOffset::new(row, stride),
                site(visit),
            )));
        }
    };
    direct(memory, start, length, visit)
}

pub(super) fn indirect_container(
    memory: &PdmaMemoryMappings,
    base: DmaAddress,
    count: u64,
) -> ModelResult<Result<(DmaRangeStart, u64), AbiFault>> {
    let visit = AbiOperandVisit::BroadcastList;
    let Some(extent) = count.checked_mul(8) else {
        return Ok(Err(AbiFault::range_overflow_indirect(base, count)));
    };
    let Ok(end) = base.checked_add(extent) else {
        return Ok(Err(AbiFault::range_overflow_indirect(base, count)));
    };
    resolve_representable(memory, base, end, extent, visit)
        .map(|result| result.map(|start| (start, extent)))
}

fn resolve_representable(
    memory: &PdmaMemoryMappings,
    start: DmaAddress,
    end: DmaAddress,
    length: u64,
    visit: AbiOperandVisit,
) -> ModelResult<Result<DmaRangeStart, AbiFault>> {
    let Some(region) = memory.region_containing(start) else {
        return Ok(Err(AbiFault::unmapped(start, length, site(visit))));
    };
    if end > region.exclusive_end() {
        return Ok(Err(AbiFault::out_of_bounds(
            region.exclusive_end(),
            AbiBounds::new(region.length(), end.raw()),
            site(visit),
        )));
    }
    range_start(region, start).map(Ok)
}

fn range_start(region: MappedDmaRegion, start: DmaAddress) -> ModelResult<DmaRangeStart> {
    let offset = start
        .raw()
        .checked_sub(region.base().raw())
        .ok_or(ModelError::DmaSubmissionState)?;
    usize::try_from(offset)
        .map(|offset| DmaRangeStart::new(region.object(), offset))
        .map_err(|_| ModelError::DmaSubmissionState)
}

const fn site(visit: AbiOperandVisit) -> AbiFaultSite {
    match visit {
        AbiOperandVisit::CopySource
        | AbiOperandVisit::BroadcastSource
        | AbiOperandVisit::VectorSourceA => {
            AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 0)
        }
        AbiOperandVisit::VectorSourceB => {
            AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 1)
        }
        AbiOperandVisit::CopyDestination | AbiOperandVisit::VectorDestination => {
            AbiFaultSite::new(AbiEventStage::Destination, AbiEventAccess::Write, 0)
        }
        AbiOperandVisit::StridedSource { row } => {
            AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, row)
        }
        AbiOperandVisit::StridedDestination { row }
        | AbiOperandVisit::BroadcastDestination { index: row } => {
            AbiFaultSite::new(AbiEventStage::Destination, AbiEventAccess::Write, row)
        }
        AbiOperandVisit::BroadcastList => {
            AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::Read, 0)
        }
    }
}
