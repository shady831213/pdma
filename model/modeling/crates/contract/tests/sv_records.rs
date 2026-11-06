//! `SystemVerilog` export CLI and codec-oracle fixtures.
#![cfg(feature = "export-rtl-layout")]
use std::process::Command;

#[test]
fn generated_records_expose_semantic_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_pdma-sv-records"))
        .output()
        .expect("run generator");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for required in [
        "package pdma_contract_records;",
        "CopyCommand_set_copy_length",
        "LinearEndpoint_set_address",
        "Event_get_info",
        "ErrorInfo_get_bytes_completed",
        "COMMANDHEADER_OPCODE_FENCE",
        "EVENT_CODE_WRITEERROR",
    ] {
        assert!(text.contains(required), "missing {required}");
    }
}

#[test]
fn emit_sv_roundtrip_against_independent_rust_codecs() {
    use pdma_model_contract::dma::*;
    let copy = Command::Copy(CopyCommand {
        id: 0x1234_5678_9abc_def0,
        copy_length: 0x0102_0304_0506_0708,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0x8877_6655_4433_2211,
            offset: 0x1020_3040_5060_7080,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0xfedc_ba98_7654_3210,
            offset: 0x1122_3344_5566_7788,
        }),
        ..CopyCommand::ZERO
    });
    let fence = Command::Fence(FenceCommand {
        header: CommandHeader {
            opcode: Opcode::Fence,
        },
        id: 0x9988_7766_5544_3322,
        word: WordWrite {
            target: WordTarget {
                address: 0x1234_5678_8765_4320,
                context: 0,
                width: WordTargetWidth::Bits64,
            },
            value: 0xf0e0_d0c0_b0a0_9080,
        },
    });
    let event = Event::stopping(
        0xfedc_ba98_7654_3210,
        EventCode::WriteError,
        0x0123_4567_89ab_cdef,
    );
    let literal = |bytes: &[u8]| {
        use std::fmt::Write;
        let mut hex = String::new();
        for byte in bytes.iter().rev() {
            write!(hex, "{byte:02x}").unwrap();
        }
        format!("{}'h{hex}", bytes.len() * 8)
    };
    let bench = include_str!("sv_records/roundtrip.sv")
        .replace("@COPY@", &literal(&copy.to_bytes().unwrap()))
        .replace("@FENCE@", &literal(&fence.to_bytes().unwrap()))
        .replace("@EVENT@", &literal(&event.to_bytes().unwrap()));
    let destination = std::env::var_os("PDMA_RECORDS_EVIDENCE_DIR").map_or_else(
        || std::env::temp_dir().join("pdma-record-codecs"),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("records_roundtrip.sv"), bench).unwrap();
}
