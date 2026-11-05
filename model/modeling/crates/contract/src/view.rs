use crate::hw_components::control::ControlConfiguration;
use crate::hw_components::control::ControlNotificationRegisters;
use crate::hw_components::control::ControlRegisters;
use crate::hw_components::windows::ChannelWindowBlock;
use crate::{
    dma::{DmaRegistersLayout, DmaRegistersRegisterId},
    hw_components::{
        context::{ContextTableRegistersLayout, ContextTableRegistersRegisterId},
        control::ControlRegistersRegisterId,
        firmware::{FirmwareRegisters, FirmwareRegistersRegisterId},
        windows::{ChannelWindowRegistersLayout, ChannelWindowRegistersRegisterId},
    },
};
use hw_model_schema::{
    Access, RegisterMapError, ResolvedRegisterMap, register_map::AccessProjectionError,
};
use std::fmt;

/// Failure resolving a physical map or applying a role's access permissions.
#[derive(Debug)]
pub enum RegisterViewError<I> {
    /// Invalid physical register layout.
    Layout(RegisterMapError<I>),
    /// Role permissions exceed the source register's permissions.
    Access(AccessProjectionError),
}
impl<I: fmt::Debug> fmt::Display for RegisterViewError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::Access(error) => error.fmt(f),
        }
    }
}
impl<I: fmt::Debug + 'static> std::error::Error for RegisterViewError<I> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Access(error) => Some(error),
        }
    }
}
impl<I> From<RegisterMapError<I>> for RegisterViewError<I> {
    fn from(error: RegisterMapError<I>) -> Self {
        Self::Layout(error)
    }
}
impl<I> From<AccessProjectionError> for RegisterViewError<I> {
    fn from(error: AccessProjectionError) -> Self {
        Self::Access(error)
    }
}

/// Software domain whose static register permissions are exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterView {
    /// Host transactions remain subject to the dynamic DMA access window.
    Host,
    /// Firmware management and DMA transactions.
    Firmware,
}

impl RegisterView {
    /// Resolves the complete DMA aperture with this role's static permissions.
    pub fn dma(
        self,
        layout: &DmaRegistersLayout,
    ) -> Result<
        ResolvedRegisterMap<DmaRegistersRegisterId>,
        RegisterViewError<DmaRegistersRegisterId>,
    > {
        Ok(layout
            .resolve()?
            .project_access(|r| self.dma_access(r.id(), r.definition().access()))?)
    }

    /// Resolves the firmware boot report as seen by this role.
    pub fn boot(
        self,
    ) -> Result<
        ResolvedRegisterMap<FirmwareRegistersRegisterId>,
        RegisterViewError<FirmwareRegistersRegisterId>,
    > {
        Ok(FirmwareRegisters::resolve()?.project_access(|_| Some(self.boot_access()))?)
    }

    /// Resolves control-channel configuration and notifications for this role.
    pub fn control(
        self,
    ) -> Result<
        ResolvedRegisterMap<ControlRegistersRegisterId>,
        RegisterViewError<ControlRegistersRegisterId>,
    > {
        Ok(ControlRegisters::resolve()?.project_access(|r| self.control_access(r.offset()))?)
    }

    /// Projects channel-window leaves while retaining the physical bank geometry.
    pub fn windows(
        self,
        layout: &ChannelWindowRegistersLayout,
    ) -> Result<
        ResolvedRegisterMap<ChannelWindowRegistersRegisterId>,
        RegisterViewError<ChannelWindowRegistersRegisterId>,
    > {
        Ok(layout.resolve()?.project_access(|register| {
            let (_, offset) = layout.windows().locate_instance(register.offset())?;
            self.window_access(offset)
        })?)
    }

    /// Resolves firmware context administration; Host has no view of this aperture.
    pub fn context_table(
        self,
        layout: &ContextTableRegistersLayout,
    ) -> Result<
        Option<ResolvedRegisterMap<ContextTableRegistersRegisterId>>,
        RegisterViewError<ContextTableRegistersRegisterId>,
    > {
        match self {
            Self::Host => Ok(None),
            Self::Firmware => Ok(Some(layout.resolve()?)),
        }
    }

    /// Projects the DMA layout without exposing administrative blocks to managed Host software.
    pub const fn dma_access(
        self,
        id: crate::dma::DmaRegistersRegisterId,
        access: Access,
    ) -> Option<Access> {
        use crate::dma::DmaRegistersRegisterId::{Admin, Channels, Discovery};
        match (self, id) {
            (Self::Firmware, _) | (Self::Host, Discovery | Channels) => Some(access),
            (Self::Host, Admin) => None,
        }
    }

    /// Access to an exact register start within one channel permission block.
    pub const fn window_access(self, offset: usize) -> Option<Access> {
        match self {
            Self::Host if offset == ChannelWindowBlock::STATUS.offset() => Some(Access::ReadOnly),
            Self::Firmware
                if offset == ChannelWindowBlock::CONTROL.offset()
                    || offset
                        == ChannelWindowBlock::CONTROL.offset()
                            + crate::hw_components::windows::WindowControl::OWNER.offset() =>
            {
                Some(Access::ReadWrite)
            }
            Self::Host | Self::Firmware => None,
        }
    }

    /// Access to the atomic firmware boot report.
    pub const fn boot_access(self) -> Access {
        match self {
            Self::Host => Access::ReadOnly,
            Self::Firmware => Access::ReadWrite,
        }
    }

    /// Access to an exact relative control-register start; absent means hidden.
    pub const fn control_access(self, offset: usize) -> Option<Access> {
        let attributes = offset
            >= ControlRegisters::CONFIG.offset() + ControlConfiguration::ATTRIBUTES.offset()
            && offset
                < ControlRegisters::CONFIG.offset()
                    + ControlConfiguration::ATTRIBUTES.offset()
                    + crate::hw_components::context::ContextAttributeRegisters::EXTENT;
        match self {
            Self::Host => {
                if offset
                    == ControlRegisters::CONFIG.offset() + ControlConfiguration::BOOTSTRAP.offset()
                    || offset
                        == ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset()
                    || attributes
                {
                    Some(Access::ReadWrite)
                } else if offset
                    == ControlRegisters::NOTIFICATION.offset()
                        + ControlNotificationRegisters::EVENT_ERROR.offset()
                {
                    Some(Access::ReadOnly)
                } else if offset == ControlRegisters::REQUEST.offset() {
                    Some(Access::WriteOnly)
                } else {
                    None
                }
            }
            Self::Firmware => {
                if offset
                    == ControlRegisters::CONFIG.offset() + ControlConfiguration::BOOTSTRAP.offset()
                    || offset
                        == ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset()
                    || attributes
                {
                    Some(Access::ReadOnly)
                } else if offset
                    == ControlRegisters::NOTIFICATION.offset()
                        + ControlNotificationRegisters::EVENT_ERROR.offset()
                {
                    Some(Access::ReadWrite)
                } else if offset
                    == ControlRegisters::NOTIFICATION.offset()
                        + ControlNotificationRegisters::RESPONSE.offset()
                    || offset
                        == ControlRegisters::NOTIFICATION.offset()
                            + ControlNotificationRegisters::EVENT.offset()
                {
                    Some(Access::WriteOnly)
                } else {
                    None
                }
            }
        }
    }
}
