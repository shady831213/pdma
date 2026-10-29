//! Optional built-in artifact renderers.

/// Generic C11 record and register-map fragment rendering.
#[cfg(feature = "export-c-header")]
pub mod c_header;

/// Veryl layout packages, including parameterized register-map geometry.
#[cfg(feature = "export-veryl-layout")]
pub mod veryl;
