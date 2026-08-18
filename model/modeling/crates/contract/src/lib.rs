//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains identifiers, records, and configuration
//! contracts only. It does not provide memory backing storage or address
//! translation behavior.
//!
//! Consumers import model vocabulary through [`semantic`], fixed wire and MMIO
//! vocabulary through [`abi`], and cross-layer conversions through [`bridge`].

pub mod abi;
pub mod bridge;
pub mod semantic;
