//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains identifiers, records, and configuration
//! contracts only. It does not provide memory backing storage or runtime
//! address-projection behavior.
//!
//! Consumers import model vocabulary through [`semantic`], fixed wire and MMIO
//! vocabulary through [`abi`], and runtime-bound associations through [`binding`].

pub mod abi;
pub mod binding;
pub mod semantic;
