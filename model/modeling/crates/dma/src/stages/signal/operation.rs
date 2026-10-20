use hardware_interface_runtime::{
    AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate, AtomicWriteOrder,
};
use pdma_model_contract::dma::{
    SignalCommand, SignalErrorReason, SignalOperation, SignalOrder, SignalTarget, WaitCommand,
    WaitCondition, WaitOrder,
};

pub(super) struct Access {
    pub(super) target: SignalTarget,
    pub(super) operation: AtomicOperation<u64>,
    pub(super) condition: Option<(WaitCondition, u64)>,
}

pub(super) const fn signal(command: SignalCommand) -> Result<Access, SignalErrorReason> {
    let operation = match command.operation {
        SignalOperation::Load => AtomicOperation::Load(match command.order {
            SignalOrder::Relaxed => AtomicReadOrder::Relaxed,
            SignalOrder::Acquire => AtomicReadOrder::Acquire,
            SignalOrder::Release | SignalOrder::AcquireRelease => {
                return Err(SignalErrorReason::InvalidOperation);
            }
        }),
        SignalOperation::Store => AtomicOperation::Store {
            value: command.value,
            order: match command.order {
                SignalOrder::Relaxed => AtomicWriteOrder::Relaxed,
                SignalOrder::Release => AtomicWriteOrder::Release,
                SignalOrder::Acquire | SignalOrder::AcquireRelease => {
                    return Err(SignalErrorReason::InvalidOperation);
                }
            },
        },
        SignalOperation::Add
        | SignalOperation::Subtract
        | SignalOperation::And
        | SignalOperation::Or
        | SignalOperation::Xor
        | SignalOperation::Exchange
        | SignalOperation::CompareExchange => {
            let update = match command.operation {
                SignalOperation::Add => AtomicUpdate::Add(command.value),
                SignalOperation::Subtract => AtomicUpdate::Subtract(command.value),
                SignalOperation::And => AtomicUpdate::And(command.value),
                SignalOperation::Or => AtomicUpdate::Or(command.value),
                SignalOperation::Xor => AtomicUpdate::Xor(command.value),
                SignalOperation::Exchange => AtomicUpdate::Exchange(command.value),
                SignalOperation::CompareExchange => AtomicUpdate::CompareExchange {
                    current: command.expected,
                    new: command.value,
                },
                SignalOperation::Load | SignalOperation::Store => {
                    return Err(SignalErrorReason::InvalidOperation);
                }
            };
            let order = match command.order {
                SignalOrder::Relaxed => AtomicOrder::Relaxed,
                SignalOrder::Acquire => AtomicOrder::Acquire,
                SignalOrder::Release => AtomicOrder::Release,
                SignalOrder::AcquireRelease => AtomicOrder::AcquireRelease,
            };
            AtomicOperation::Update { update, order }
        }
    };
    Ok(Access {
        target: command.target,
        operation,
        condition: None,
    })
}

pub(super) const fn wait(command: WaitCommand) -> Access {
    Access {
        target: command.target,
        operation: AtomicOperation::Load(match command.order {
            WaitOrder::Relaxed => AtomicReadOrder::Relaxed,
            WaitOrder::Acquire => AtomicReadOrder::Acquire,
        }),
        condition: Some((command.condition, command.compare)),
    }
}
