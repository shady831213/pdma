//! Optional built-in artifact renderers.

/// Generic C11 record and register-map fragment rendering.
#[cfg(feature = "export-c-header")]
pub mod c_header;

/// Veryl layout packages, including parameterized register-map geometry.
#[cfg(feature = "export-veryl-layout")]
pub mod veryl;

#[cfg(any(feature = "export-sv", feature = "export-uvm-regs"))]
mod sv;
/// SystemVerilog byte records and register layout packages.
#[cfg(feature = "export-sv")]
pub mod system_verilog;
/// UVM register abstraction layer classes.
#[cfg(feature = "export-uvm-regs")]
pub mod uvm;

#[cfg(any(feature = "export-veryl-layout", feature = "export-sv"))]
mod layout;
