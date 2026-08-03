use std::fmt::Write as _;

use super::super::mmio::{
    GlobalCommand, IrqCommand, IrqStateBit, MMIO_REGISTERS, MmioAccess, MmioReset, MmioResult,
    QueueCommand,
};

pub(super) fn push(output: &mut String) {
    push_access_and_reset(output);
    for register in MMIO_REGISTERS.iter() {
        let prefix = format!("PDMA_MODEL_MMIO_{}", register.name());
        push_u32(output, &format!("{prefix}_OFFSET"), register.offset());
        let _ = writeln!(
            output,
            "#define {prefix}_WIDTH UINT32_C({})",
            register.width()
        );
        push_u32(
            output,
            &format!("{prefix}_ACCESS"),
            register.access().code(),
        );
        push_u32(
            output,
            &format!("{prefix}_RESET_CLASS"),
            register.reset().code(),
        );
        push_u64(
            output,
            &format!("{prefix}_RESET_VALUE"),
            register.reset_value(),
        );
        push_u64(
            output,
            &format!("{prefix}_WRITABLE_MASK"),
            register.writable_mask(),
        );
    }
    output.push('\n');
    for command in GlobalCommand::ALL {
        push_u32(
            output,
            &format!("PDMA_MODEL_MMIO_GLOBAL_COMMAND_{}", command.name()),
            command.code(),
        );
    }
    for command in QueueCommand::ALL {
        push_u32(
            output,
            &format!("PDMA_MODEL_MMIO_QUEUE_COMMAND_{}", command.name()),
            command.code(),
        );
    }
    for command in IrqCommand::ALL {
        push_u32(
            output,
            &format!("PDMA_MODEL_MMIO_IRQ_COMMAND_{}", command.name()),
            command.code(),
        );
    }
    for result in MmioResult::ALL {
        push_u32(
            output,
            &format!("PDMA_MODEL_MMIO_RESULT_{}", result.name()),
            result.code(),
        );
    }
    for bit in IrqStateBit::ALL {
        push_u32(
            output,
            &format!("PDMA_MODEL_MMIO_IRQ_STATE_{}", bit.name()),
            bit.mask(),
        );
    }
    output.push('\n');
}

fn push_access_and_reset(output: &mut String) {
    push_u32(
        output,
        "PDMA_MODEL_MMIO_ACCESS_RO",
        MmioAccess::ReadOnly.code(),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_ACCESS_RW",
        MmioAccess::ReadWrite.code(),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_ACCESS_WO",
        MmioAccess::WriteOnly.code(),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_RESET_CONST",
        MmioReset::Constant.code(),
    );
    push_u32(output, "PDMA_MODEL_MMIO_RESET_R0", MmioReset::Zero.code());
    push_u32(
        output,
        "PDMA_MODEL_MMIO_RESET_POST",
        MmioReset::PostReset.code(),
    );
    output.push('\n');
}

fn push_u32(output: &mut String, name: &str, value: u32) {
    let _ = writeln!(output, "#define {name} UINT32_C({value})");
}

fn push_u64(output: &mut String, name: &str, value: u64) {
    let _ = writeln!(output, "#define {name} UINT64_C({value})");
}
