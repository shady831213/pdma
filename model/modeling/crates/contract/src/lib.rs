//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains hardware identifiers, records, and register
//! definitions only. It does not provide memory backing storage or runtime
//! address-projection behavior.
//!
//! Consumers import model vocabulary through [`semantic`] and fixed wire and MMIO
//! vocabulary through [`abi`].

pub mod abi;
pub mod semantic;
