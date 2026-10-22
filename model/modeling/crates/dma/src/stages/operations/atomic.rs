use super::{Failure, Progress, access_failure, word::geometry};
use crate::{MemoryFailure, SelectedResources};
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate, MemoryAccess,
};
use pdma_model_contract::dma::{
    AtomicCommand, AtomicCommandOperandsValues, AtomicCommandOrder, EventCode, SemaphoreCommand,
    SemaphoreCommandOperation, WordTarget, WordTargetWidth,
};

fn failure<E: Into<MemoryFailure>>(error: AtomicAccessError<E>) -> Failure {
    match error {
        AtomicAccessError::Unsupported => Failure::Stop(EventCode::Unsupported),
        AtomicAccessError::Access(error) => access_failure(error, EventCode::WriteError),
    }
}

fn supported<M, SB>(
    context: &SelectedResources<'_, M, SB>,
    target: WordTarget,
) -> Result<(), Failure> {
    geometry(target)?;
    let supported = match target.width {
        WordTargetWidth::Bits32 => context.capabilities.atomic32(),
        WordTargetWidth::Bits64 => context.capabilities.atomic64(),
    };
    if supported {
        Ok(())
    } else {
        Err(Failure::Stop(EventCode::Unsupported))
    }
}

fn update<T>(operands: AtomicCommandOperandsValues, convert: fn(u64) -> T) -> AtomicUpdate<T> {
    match operands {
        AtomicCommandOperandsValues::Add(value) => AtomicUpdate::Add(convert(value.value)),
        AtomicCommandOperandsValues::Subtract(value) => {
            AtomicUpdate::Subtract(convert(value.value))
        }
        AtomicCommandOperandsValues::And(value) => AtomicUpdate::And(convert(value.value)),
        AtomicCommandOperandsValues::Or(value) => AtomicUpdate::Or(convert(value.value)),
        AtomicCommandOperandsValues::Xor(value) => AtomicUpdate::Xor(convert(value.value)),
        AtomicCommandOperandsValues::Exchange(value) => {
            AtomicUpdate::Exchange(convert(value.value))
        }
        AtomicCommandOperandsValues::CompareExchange(value) => AtomicUpdate::CompareExchange {
            current: convert(value.compare),
            new: convert(value.value),
        },
    }
}

pub(super) fn execute<M: MemoryAccess<SB>, SB>(
    context: &SelectedResources<'_, M, SB>,
    command: AtomicCommand,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    supported(context, command.target)?;
    let (address, _, _) = geometry(command.target)?;
    let memory = context.memory_for(context.work(), command.target.context);
    let order = match command.order {
        AtomicCommandOrder::Relaxed => AtomicOrder::Relaxed,
        AtomicCommandOrder::Acquire => AtomicOrder::Acquire,
        AtomicCommandOrder::Release => AtomicOrder::Release,
        AtomicCommandOrder::AcquireRelease => AtomicOrder::AcquireRelease,
    };
    match command.target.width {
        WordTargetWidth::Bits32 => {
            let update = update(command.operands, |value| {
                let [a, b, c, d, _, _, _, _] = value.to_le_bytes();
                u32::from_le_bytes([a, b, c, d])
            });
            memory
                .atomic32(address, AtomicOperation::Update { update, order })
                .map_err(failure)?;
        }
        WordTargetWidth::Bits64 => {
            memory
                .atomic64(
                    address,
                    AtomicOperation::Update {
                        update: update(command.operands, |value| value),
                        order,
                    },
                )
                .map_err(failure)?;
        }
    }
    Ok(Progress::Done)
}

pub(super) fn semaphore<M: MemoryAccess<SB>, SB>(
    context: &SelectedResources<'_, M, SB>,
    command: SemaphoreCommand,
) -> Result<Progress, Failure>
where
    M::Error: Into<MemoryFailure>,
{
    supported(context, command.target)?;
    match command.target.width {
        WordTargetWidth::Bits32 => semaphore32(context, command),
        WordTargetWidth::Bits64 => semaphore64(context, command),
    }
}

macro_rules! semaphore_word {
    ($name:ident, $atomic:ident, $word:ty) => {
        fn $name<M: MemoryAccess<SB>, SB>(
            context: &SelectedResources<'_, M, SB>,
            command: SemaphoreCommand,
        ) -> Result<Progress, Failure>
        where
            M::Error: Into<MemoryFailure>,
        {
            let (address, _, _) = geometry(command.target)?;
            let memory = context.memory_for(context.work(), command.target.context);
            let old = memory
                .$atomic(address, AtomicOperation::Load(AtomicReadOrder::Relaxed))
                .map_err(failure)?;
            let (new, order) = match command.operation {
                SemaphoreCommandOperation::Signal => (
                    old.checked_add(1)
                        .ok_or(Failure::Stop(EventCode::ArithmeticOverflow))?,
                    AtomicOrder::Release,
                ),
                SemaphoreCommandOperation::Wait => {
                    if old == 0 {
                        return Ok(Progress::Yield);
                    }
                    (old - 1, AtomicOrder::Acquire)
                }
            };
            let observed = memory
                .$atomic(
                    address,
                    AtomicOperation::Update {
                        update: AtomicUpdate::<$word>::CompareExchange { current: old, new },
                        order,
                    },
                )
                .map_err(failure)?;
            Ok(if observed == old {
                Progress::Done
            } else {
                Progress::Yield
            })
        }
    };
}

semaphore_word!(semaphore32, atomic32, u32);
semaphore_word!(semaphore64, atomic64, u64);
