use super::{Failure, Progress, atomic, fill, word};
use crate::{Error, MemoryFailure, SelectedResources};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{
    AtomicCommand, FillCommand, SemaphoreCommand, TrapCommand, WaitCommand,
};
use pdma_model_contract::dma::{CommandPayloadValues, WordWrite};

pub(super) trait Operand: Copy {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error>;
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>;
}

#[derive(Clone, Copy, Debug)]
pub(in crate::stages) struct Write(WordWrite);

impl Operand for Write {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Write(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(Self(operand))
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        word::write(context, self.0, false)
    }
}

impl Operand for FillCommand {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Fill(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(operand)
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        fill::execute(context, self)
    }
}

impl Operand for WaitCommand {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Wait(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(operand)
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        word::wait(context, self)
    }
}

impl Operand for AtomicCommand {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Atomic(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(operand)
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        atomic::execute(context, self)
    }
}

impl Operand for SemaphoreCommand {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Semaphore(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(operand)
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        atomic::semaphore(context, self)
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::stages) struct Fence(WordWrite);

impl Operand for Fence {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Fence(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(Self(operand))
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        word::write(context, self.0, true)
    }
}

impl Operand for TrapCommand {
    fn project(payload: CommandPayloadValues) -> Result<Self, Error> {
        let CommandPayloadValues::Trap(operand) = payload else {
            return Err(Error::Routing);
        };
        Ok(operand)
    }
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>,
    {
        context.selected.trap();
        Ok(Progress::Done)
    }
}
