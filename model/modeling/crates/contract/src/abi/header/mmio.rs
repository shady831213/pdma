use std::fmt::Write as _;

use hardware_interface_schema::{
    export::backends::c_header::RegisterMapCHeaderModel,
    register::{Access, ResetKind},
    register_map::ResolvedRegisterMap,
};

use super::super::mmio::{
    GlobalCommand, IrqCommand, IrqStateBit, MmioAccess, MmioReset, MmioResult,
    PdmaMmioMapRegisterId, QueueCommand,
};

pub(super) fn push(output: &mut String, schema: &ResolvedRegisterMap<PdmaMmioMapRegisterId>) {
    let model = RegisterMapCHeaderModel::from_resolved(schema);
    push_access_and_reset(output);
    for (register, resolved) in model.fixed().iter().zip(schema.registers()) {
        let name = resolved
            .path()
            .split('.')
            .skip(1)
            .collect::<Vec<_>>()
            .join("_")
            .to_ascii_uppercase();
        let prefix = format!("PDMA_MODEL_MMIO_{name}");
        push_usize(output, &format!("{prefix}_OFFSET"), register.offset());
        let _ = writeln!(
            output,
            "#define {prefix}_WIDTH UINT32_C({})",
            register.width()
        );
        push_u32(
            output,
            &format!("{prefix}_ACCESS"),
            access_code(register.access()),
        );
        push_u32(
            output,
            &format!("{prefix}_RESET_CLASS"),
            reset_code(register.reset_kind()),
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
        access_code(MmioAccess::ReadOnly),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_ACCESS_RW",
        access_code(MmioAccess::ReadWrite),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_ACCESS_WO",
        access_code(MmioAccess::WriteOnly),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_RESET_CONST",
        reset_code(MmioReset::Constant),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_RESET_R0",
        reset_code(MmioReset::Zero),
    );
    push_u32(
        output,
        "PDMA_MODEL_MMIO_RESET_POST",
        reset_code(MmioReset::PostReset),
    );
    output.push('\n');
}

fn push_u32(output: &mut String, name: &str, value: u32) {
    let _ = writeln!(output, "#define {name} UINT32_C({value})");
}

fn push_usize(output: &mut String, name: &str, value: usize) {
    let _ = writeln!(output, "#define {name} UINT32_C({value})");
}

fn push_u64(output: &mut String, name: &str, value: u64) {
    let _ = writeln!(output, "#define {name} UINT64_C({value})");
}

const fn access_code(access: Access) -> u32 {
    match access {
        Access::ReadOnly => 0,
        Access::ReadWrite => 1,
        Access::WriteOnly => 2,
    }
}

const fn reset_code(reset: ResetKind) -> u32 {
    match reset {
        ResetKind::Constant => 0,
        ResetKind::Zero => 1,
        ResetKind::PostReset => 2,
    }
}
