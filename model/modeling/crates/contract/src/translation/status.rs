use crate::abi::AbiControlError;
use crate::semantic::DeviceRunState;

impl TryFrom<u16> for DeviceRunState {
    type Error = AbiControlError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Running),
            2 => Ok(Self::Blocked),
            _ => Err(AbiControlError::UnknownStatusValue { value }),
        }
    }
}
