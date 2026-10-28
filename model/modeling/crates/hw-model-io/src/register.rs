mod access;
mod address;
pub mod observation;

pub use access::{AddressedRegisterAccess, RegisterAccess};
pub use address::{
    RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset, RegisterBusWidth,
};
