use super::{RegisterSpec, ResolvedRegisterMap};
use crate::Access;

/// A projection attempted to grant access absent from its source definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessProjectionError {
    /// Source register name.
    pub register: &'static str,
    /// Source access permission.
    pub original: Access,
    /// Requested incompatible permission.
    pub requested: Access,
}

impl core::fmt::Display for AccessProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "{}: cannot project {:?} as {:?}",
            self.register, self.original, self.requested
        )
    }
}

impl std::error::Error for AccessProjectionError {}

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegisterMap<I, F> {
    /// Filters or restricts leaf access without changing addresses or map geometry.
    /// The same policy applies to bank templates and each expanded instance.
    pub fn project_access(
        &self,
        policy: impl Fn(RegisterSpec<I, F>) -> Option<Access>,
    ) -> Result<Self, AccessProjectionError> {
        let project = |register: RegisterSpec<I, F>| {
            let Some(requested) = policy(register) else {
                return Ok(None);
            };
            let original = register.access();
            if original != Access::ReadWrite && original != requested {
                return Err(AccessProjectionError {
                    register: register.name(),
                    original,
                    requested,
                });
            }
            let register = register.with_access(requested);
            Ok(Some(match requested {
                Access::ReadOnly => register.with_writable_mask(0),
                Access::ReadWrite | Access::WriteOnly => register,
            }))
        };
        let mut view = self.clone();
        let mut error = None;
        view.registers
            .retain_mut(|leaf| match project(leaf.register) {
                Ok(Some(register)) => {
                    leaf.register = register;
                    true
                }
                Ok(None) => false,
                Err(failure) => {
                    error = Some(failure);
                    false
                }
            });
        for bank in &mut view.banks {
            bank.registers = bank
                .registers
                .iter()
                .copied()
                .filter_map(|register| project(register).transpose())
                .collect::<Result<_, _>>()?;
            bank.expanded_registers
                .retain_mut(|leaf| match project(leaf.register) {
                    Ok(Some(register)) => {
                        leaf.register = register;
                        true
                    }
                    Ok(None) => false,
                    Err(failure) => {
                        error = Some(failure);
                        false
                    }
                });
        }
        error.map_or(Ok(view), Err)
    }
}
