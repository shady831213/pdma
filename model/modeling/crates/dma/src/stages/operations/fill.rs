use super::{Failure, Progress, access_failure};
use crate::{MemoryFailure, SelectedResources};
use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{EventCode, FillCommand, FillCommandWidth};

pub(super) fn execute<M: MemoryAccess<SB>, SB>(
    context: &mut SelectedResources<'_, M, SB>,
    fill: FillCommand,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    let completed = context
        .selected
        .work
        .as_ref()
        .map_or(0, |accepted| accepted.fill_completed);
    fill.address
        .checked_add(fill.length)
        .ok_or(Failure::Stop(EventCode::InvalidLength))?;
    let remaining = fill
        .length
        .checked_sub(completed)
        .ok_or(Failure::Stop(EventCode::InvalidLength))?;
    if remaining == 0 {
        return Ok(Progress::Done);
    }
    let width = match fill.width {
        FillCommandWidth::Byte => 1,
        FillCommandWidth::Bits32 => 4,
        FillCommandWidth::Bits64 => 8,
    };
    let length = usize::try_from(remaining)
        .unwrap_or(crate::COPY_CHUNK_BYTES)
        .min(crate::COPY_CHUNK_BYTES);
    let pattern = fill.pattern.to_le_bytes();
    let phase = usize::try_from(
        completed % u64::try_from(width).map_err(|_| Failure::Stop(EventCode::InvalidOperation))?,
    )
    .map_err(|_| Failure::Stop(EventCode::InvalidOperation))?;
    let mut bytes = [0; crate::COPY_CHUNK_BYTES];
    for (index, byte) in bytes.iter_mut().take(length).enumerate() {
        *byte = *pattern
            .get((phase + index) % width)
            .ok_or(Failure::Stop(EventCode::InvalidOperation))?;
    }
    context
        .memory_for(context.work(), fill.context)
        .write(
            MemoryAddress::new(fill.address + completed),
            MemoryAlignment::ONE,
            bytes
                .get(..length)
                .ok_or(Failure::Stop(EventCode::InvalidLength))?,
        )
        .map_err(|error| access_failure(error, EventCode::WriteError))?;
    let accepted = context
        .selected
        .work
        .as_mut()
        .ok_or(Failure::Stop(EventCode::InvalidOperation))?;
    accepted.fill_completed +=
        u64::try_from(length).map_err(|_| Failure::Stop(EventCode::InvalidLength))?;
    Ok(if accepted.fill_completed == fill.length {
        Progress::Done
    } else {
        Progress::More
    })
}
