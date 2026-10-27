use super::PlatformConfigError;
use crate::harness::runtime::platform::MemoryRequest;
use hardware_interface_memory::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};
use pdma_model_dma::MemoryFailure;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::harness) enum SpaceError<E> {
    MissingSpace {
        identity: MemoryIdentity,
        address: MemoryAddress,
    },
    Downstream(E),
}

impl<E: fmt::Display> fmt::Display for SpaceError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSpace { identity, address } => write!(
                f,
                "missing memory space {identity:?} at {:#x}",
                address.raw()
            ),
            Self::Downstream(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SpaceError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingSpace { .. } => None,
            Self::Downstream(error) => Some(error),
        }
    }
}

impl<E: Into<Self>> From<SpaceError<E>> for MemoryFailure {
    fn from(error: SpaceError<E>) -> Self {
        match error {
            SpaceError::MissingSpace { .. } => Self::Access,
            SpaceError::Downstream(error) => error.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::harness::runtime) struct FunctionSpaces<M> {
    pub(in crate::harness::runtime) backing: Vec<(MemoryIdentity, M)>,
    pub(in crate::harness::runtime) bind: Option<fn(&M, MemoryRequest) -> M>,
}

impl<M> FunctionSpaces<M> {
    pub(in crate::harness::runtime) fn new(spaces: Vec<M>) -> Result<Self, PlatformConfigError> {
        Ok(Self {
            backing: spaces
                .into_iter()
                .enumerate()
                .map(|(index, memory)| {
                    Ok((
                        MemoryIdentity {
                            function: FunctionId::new(
                                u32::try_from(index)
                                    .map_err(|_| PlatformConfigError::FunctionCount)?,
                            ),
                            attributes: ContextAttributes::DEFAULT,
                        },
                        memory,
                    ))
                })
                .collect::<Result<_, PlatformConfigError>>()?,
            bind: None,
        })
    }
}

impl<M: MemoryAccess> FunctionSpaces<M> {
    fn get(
        &self,
        identity: MemoryIdentity,
        address: MemoryAddress,
    ) -> Result<&M, SpaceError<M::Error>> {
        self.backing
            .iter()
            .find(|(key, _)| *key == identity)
            .map(|(_, memory)| memory)
            .ok_or(SpaceError::MissingSpace { identity, address })
    }
}

impl<M: MemoryAccess> MemoryAccess<MemoryRequest> for FunctionSpaces<M> {
    type Error = SpaceError<M::Error>;

    fn atomic32_with_sideband(
        &self,
        request: &MemoryRequest,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        let memory = self
            .get(request.identity, address)
            .map_err(AtomicAccessError::Access)?;
        let bound = self.bind.map(|bind| bind(memory, *request));
        bound
            .as_ref()
            .unwrap_or(memory)
            .atomic32(address, operation)
            .map_err(|error| match error {
                AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                AtomicAccessError::Access(error) => {
                    AtomicAccessError::Access(SpaceError::Downstream(error))
                }
            })
    }

    fn atomic64_with_sideband(
        &self,
        request: &MemoryRequest,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        let memory = self
            .get(request.identity, address)
            .map_err(AtomicAccessError::Access)?;
        let bound = self.bind.map(|bind| bind(memory, *request));
        bound
            .as_ref()
            .unwrap_or(memory)
            .atomic64(address, operation)
            .map_err(|error| match error {
                AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                AtomicAccessError::Access(error) => {
                    AtomicAccessError::Access(SpaceError::Downstream(error))
                }
            })
    }

    fn validate_with_sideband(
        &self,
        request: &MemoryRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        let memory = self.get(request.identity, address)?;
        let bound = self.bind.map(|bind| bind(memory, *request));
        bound
            .as_ref()
            .unwrap_or(memory)
            .validate(address, alignment, length)
            .map_err(SpaceError::Downstream)
    }

    fn read_with_sideband(
        &self,
        request: &MemoryRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        let memory = self.get(request.identity, address)?;
        let bound = self.bind.map(|bind| bind(memory, *request));
        bound
            .as_ref()
            .unwrap_or(memory)
            .read(address, alignment, bytes)
            .map_err(SpaceError::Downstream)
    }

    fn write_with_sideband(
        &self,
        request: &MemoryRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        let memory = self.get(request.identity, address)?;
        let bound = self.bind.map(|bind| bind(memory, *request));
        bound
            .as_ref()
            .unwrap_or(memory)
            .write(address, alignment, bytes)
            .map_err(SpaceError::Downstream)
    }
}

impl<M: MemoryAccess> MemoryAccess<MemoryIdentity> for FunctionSpaces<M> {
    type Error = SpaceError<M::Error>;

    fn validate_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.validate_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            length,
        )
    }

    fn read_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.read_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            bytes,
        )
    }

    fn write_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.write_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            bytes,
        )
    }
}
