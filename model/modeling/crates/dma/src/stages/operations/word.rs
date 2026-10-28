use super::{Failure, Progress, access_failure};
use crate::{MemoryFailure, SelectedResources};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    EventCode, TimestampCommand, WaitCommand, WaitCommandComparison, WordTarget, WordTargetWidth,
    WordWrite,
};

pub(super) fn geometry(
    target: WordTarget,
) -> Result<(MemoryAddress, MemoryAlignment, usize), Failure> {
    let width = match target.width {
        WordTargetWidth::Bits32 => 4,
        WordTargetWidth::Bits64 => 8,
    };
    if !target.address.is_multiple_of(width) || target.address.checked_add(width).is_none() {
        return Err(Failure::Stop(EventCode::InvalidOperation));
    }
    Ok((
        MemoryAddress::new(target.address),
        MemoryAlignment::try_new(width).map_err(|_| Failure::Stop(EventCode::InvalidOperation))?,
        usize::try_from(width).map_err(|_| Failure::Stop(EventCode::InvalidOperation))?,
    ))
}

pub(super) fn write<M: MemoryAccess<SB>, SB>(
    context: &SelectedResources<'_, M, SB>,
    write: WordWrite,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    let (address, alignment, width) = geometry(write.target)?;
    let bytes = write.value.to_le_bytes();
    context
        .memory_for(context.work(), write.target.context)
        .write(
            address,
            alignment,
            bytes
                .get(..width)
                .ok_or(Failure::Stop(EventCode::InvalidOperation))?,
        )
        .map_err(|error| access_failure(error, EventCode::WriteError))?;
    Ok(Progress::Done)
}

pub(super) fn timestamp<M: MemoryAccess<SB>, SB>(
    context: &mut SelectedResources<'_, M, SB>,
    command: TimestampCommand,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    let target = WordTarget {
        address: command.address,
        context: command.context,
        width: WordTargetWidth::Bits64,
    };
    geometry(target)?;
    let timer = context.timer.ok_or(Failure::Stop(EventCode::Unsupported))?;
    let accepted = context
        .selected
        .work
        .as_mut()
        .ok_or(Failure::Stop(EventCode::InvalidOperation))?;
    let value = *accepted.timestamp.get_or_insert_with(|| timer.sample());
    write(context, WordWrite { target, value })
}

pub(super) fn wait<M: MemoryAccess<SB>, SB>(
    context: &SelectedResources<'_, M, SB>,
    wait: WaitCommand,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    let (address, alignment, width) = geometry(wait.target)?;
    let mut bytes = [0; 8];
    context
        .memory_for(context.work(), wait.target.context)
        .read(
            address,
            alignment,
            bytes
                .get_mut(..width)
                .ok_or(Failure::Stop(EventCode::InvalidOperation))?,
        )
        .map_err(|error| access_failure(error, EventCode::ReadError))?;
    let observed = u64::from_le_bytes(bytes);
    let (mask, sign) = match wait.target.width {
        WordTargetWidth::Bits32 => (u64::from(u32::MAX), 1_u64 << 31),
        WordTargetWidth::Bits64 => (u64::MAX, 1_u64 << 63),
    };
    let value = wait.value & mask;
    let signed_observed = observed ^ sign;
    let signed_value = value ^ sign;
    let satisfied = match wait.comparison {
        WaitCommandComparison::Equal => observed == value,
        WaitCommandComparison::NotEqual => observed != value,
        WaitCommandComparison::UnsignedLess => observed < value,
        WaitCommandComparison::UnsignedLessEqual => observed <= value,
        WaitCommandComparison::UnsignedGreater => observed > value,
        WaitCommandComparison::UnsignedGreaterEqual => observed >= value,
        WaitCommandComparison::SignedLess => signed_observed < signed_value,
        WaitCommandComparison::SignedLessEqual => signed_observed <= signed_value,
        WaitCommandComparison::SignedGreater => signed_observed > signed_value,
        WaitCommandComparison::SignedGreaterEqual => signed_observed >= signed_value,
        WaitCommandComparison::Reached => observed.wrapping_sub(value) & sign == 0,
    };
    Ok(if satisfied {
        Progress::Done
    } else {
        Progress::Yield
    })
}
