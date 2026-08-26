//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains identifiers, records, and configuration
//! contracts only. It does not provide memory backing storage or address
//! translation behavior.
//!
//! Consumers import model vocabulary through [`semantic`], fixed wire and MMIO
//! vocabulary through [`abi`], runtime-bound associations through [`binding`],
//! and cross-layer value conversions through [`translation`].

pub mod abi;
pub mod binding;
pub mod semantic;
pub mod translation;
