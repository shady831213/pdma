//! Scalar copy-engine commands through actual MMIO and memory.
mod command_operations {
    mod atomic;
    mod atomic_widths;
    mod comparisons;
    mod faults;
    mod flow;
    mod maintenance;
    mod notifications;
    mod support;
}
