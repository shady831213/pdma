use super::{Failure, Progress, atomic, fill, word};
use crate::{Error, MemoryFailure, SelectedResources};
use hw_model_io::memory::MemoryAccess;
use pdma_model_contract::dma::{
    AtomicCommand, FillCommand, SemaphoreCommand, TimestampCommand, TrapCommand, WaitCommand,
};
use pdma_model_contract::dma::{Command, FenceCommand, WriteCommand};

pub(super) trait Operand: Copy {
    fn project(command: Command) -> Result<Self, Error>;
    fn id(self) -> u64;
    fn execute<M: MemoryAccess<SB>, SB>(
        self,
        context: &mut SelectedResources<'_, M, SB>,
    ) -> Result<Progress, Failure>
    where
        M::Error: Into<MemoryFailure>;
}

impl Operand for WriteCommand {
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Write(operand) = command else {
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
        word::write(context, self.word)
    }
}

impl Operand for FillCommand {
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Fill(operand) = command else {
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
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Wait(operand) = command else {
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
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Atomic(operand) = command else {
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
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Semaphore(operand) = command else {
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

impl Operand for FenceCommand {
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Fence(operand) = command else {
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
        word::write(context, self.word)
    }
}

impl Operand for TimestampCommand {
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Timestamp(operand) = command else {
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
        word::timestamp(context, self)
    }
}

impl Operand for TrapCommand {
    fn id(self) -> u64 {
        self.id
    }
    fn project(command: Command) -> Result<Self, Error> {
        let Command::Trap(operand) = command else {
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
