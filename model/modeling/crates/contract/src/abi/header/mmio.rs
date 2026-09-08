use std::fmt::Write as _;

use hardware_interface_schema::{
    register::{Access, RegisterSpec, ResetKind},
    register_map::ResolvedRegisterMap,
};

use super::super::registers::{
    IrqStateBit, PdmaRegisterMap, PdmaRegisterMapRegisterId, QueueInstanceRegisters,
};

pub(super) fn push(output: &mut String, schema: &ResolvedRegisterMap<PdmaRegisterMapRegisterId>) {
    for (name, value) in [
        ("ACCESS_RO", 0),
        ("ACCESS_RW", 1),
        ("ACCESS_WO", 2),
        ("RESET_CONST", 0),
        ("RESET_R0", 1),
        ("RESET_POST", 2),
    ] {
        push_u32(output, name, value);
    }
    for (name, value) in [
        ("QUEUE_BASE", PdmaRegisterMap::QUEUES.offset()),
        ("QUEUE_STRIDE", PdmaRegisterMap::QUEUES.stride()),
        ("IRQ_BASE", QueueInstanceRegisters::SOURCES.offset()),
        ("IRQ_STRIDE", QueueInstanceRegisters::SOURCES.stride()),
        ("IRQ_COUNT", QueueInstanceRegisters::SOURCES.count()),
    ] {
        let _ = writeln!(output, "#define PDMA_MODEL_MMIO_{name} UINT32_C({value})");
    }
    for resolved in schema.registers() {
        if resolved.instance().is_some_and(|index| index != 0) {
            continue;
        }
        let irq_base = PdmaRegisterMap::QUEUES.offset() + QueueInstanceRegisters::SOURCES.offset();
        let (block, base) = if resolved.offset() < PdmaRegisterMap::QUEUES.offset() {
            ("GLOBAL", PdmaRegisterMap::GLOBAL.offset())
        } else if resolved.offset() >= irq_base {
            if resolved.offset() >= irq_base + QueueInstanceRegisters::SOURCES.stride() {
                continue;
            }
            ("IRQ", irq_base)
        } else {
            ("QUEUE", PdmaRegisterMap::QUEUES.offset())
        };
        let register = RegisterSpec::place(
            resolved.id(),
            resolved.name(),
            resolved.offset() - base,
            resolved.definition(),
        );
        push_register(output, block, register);
    }
    for bit in IrqStateBit::ALL {
        push_u32(output, &format!("IRQ_STATE_{}", bit.name()), bit.mask());
    }
    output.push('\n');
}

fn push_register(
    output: &mut String,
    block: &str,
    register: RegisterSpec<PdmaRegisterMapRegisterId>,
) {
    let prefix = format!("{block}_{}", register.name().to_ascii_uppercase());
    for (suffix, value) in [("OFFSET", register.offset()), ("WIDTH", register.width())] {
        let _ = writeln!(
            output,
            "#define PDMA_MODEL_MMIO_{prefix}_{suffix} UINT32_C({value})"
        );
    }
    push_u32(
        output,
        &format!("{prefix}_ACCESS"),
        match register.access() {
            Access::ReadOnly => 0,
            Access::ReadWrite => 1,
            Access::WriteOnly => 2,
        },
    );
    push_u32(
        output,
        &format!("{prefix}_RESET_CLASS"),
        match register.reset_kind() {
            ResetKind::Constant => 0,
            ResetKind::Zero => 1,
            ResetKind::PostReset => 2,
        },
    );
    for (suffix, value) in [
        ("RESET_VALUE", register.reset_value()),
        ("WRITABLE_MASK", register.writable_mask()),
    ] {
        let _ = writeln!(
            output,
            "#define PDMA_MODEL_MMIO_{prefix}_{suffix} UINT64_C({value})"
        );
    }
}

fn push_u32(output: &mut String, name: &str, value: u32) {
    let _ = writeln!(output, "#define PDMA_MODEL_MMIO_{name} UINT32_C({value})");
}
