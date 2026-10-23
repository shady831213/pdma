use crate::hw_components::control::ControlConfiguration;
use crate::hw_components::control::ControlNotificationRegisters;
use crate::hw_components::control::ControlRegisters;
use crate::hw_components::windows::ChannelWindowBlock;
use hardware_interface_schema::Access;

/// Software domain whose static register permissions are exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterView {
    /// Host transactions remain subject to the dynamic DMA access window.
    Host,
    /// Firmware management and DMA transactions.
    Firmware,
}

impl RegisterView {
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
