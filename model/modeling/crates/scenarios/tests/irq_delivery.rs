#![allow(missing_docs)]

#[path = "irq_delivery/alternation.rs"]
mod alternation;
#[path = "irq_delivery/alternation_program.rs"]
pub mod alternation_program;
#[path = "irq_delivery/assertions.rs"]
pub mod assertions;
#[path = "irq_delivery/legacy.rs"]
mod legacy;
#[path = "irq_delivery/shared_vector.rs"]
mod shared_vector;
#[path = "irq_delivery/support.rs"]
pub mod support;
