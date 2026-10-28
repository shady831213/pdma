//! Strict C11 acceptance of the generated current PDMA contract.

#![cfg(feature = "export-c-header")]

mod control_queue;
mod copy_engine;
/// Independently specified DMA wire fixtures for the C consumer.
pub mod dma_wire;

use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    process::{Command as ProcessCommand, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use hw_model_schema::{Documentation, ResolvedRegisterMap};
use pdma_model_contract::control::{ControlRequest, ControlResponse};
use pdma_model_contract::dma::{Command, DmaRegisters, Event, LinearEndpoint, Segment};
use pdma_model_contract::hw_components::control::ControlRegisters;

type TestResult = Result<(), Box<dyn Error>>;
const QUEUE_BYTES: [u8; 72] = [
    0, 16, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0,
    0, 4, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 48, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 7, 0,
    0, 0, 5, 0, 0, 0, 0, 0, 0, 0,
];

const CONTROL_BYTES: [&[u8]; 4] = [
    &[
        1, 2, 3, 4, 5, 6, 7, 8, 1, 0, 0, 0, 7, 0, 0, 0, 0x40, 0x30, 0x20, 0x10, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    &[
        9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    &[
        9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    &[
        1, 2, 3, 4, 5, 6, 7, 8, 3, 0, 0, 0, 0x40, 0x30, 0x20, 0x10, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
];

#[test]
fn generated_header_preserves_resolved_contract_descriptions() -> TestResult {
    // Given: the current record and MMIO schemas used by the device.
    let records = [
        Segment::SPEC.resolve()?,
        LinearEndpoint::SPEC.resolve()?,
        Command::SPEC.resolve()?,
        Event::SPEC.resolve()?,
        ControlRequest::SPEC.resolve()?,
        ControlResponse::SPEC.resolve()?,
        pdma_model_contract::control::ControlRingState::SPEC.resolve()?,
        pdma_model_contract::control::ControlQueue::SPEC.resolve()?,
    ];
    let registers = DmaRegisters::new(3)?.resolve()?;
    let control = ControlRegisters::resolve()?;

    // When: the public generator emits its complete header.
    let temp = TempDirectory::new()?;
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3", "--write-dir"])
        .arg(&temp.0)
        .output()?;

    // Then: schema descriptions survive in the C artifact, including multiline status details.
    assert_success("header generator", &output);
    let headers = ["pdma_common.h", "pdma_host.h", "pdma_firmware.h"]
        .map(|name| fs::read_to_string(temp.0.join(name)))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let header = headers.as_str();
    for record in records {
        assert_descriptions(header, record.documentation());
        for member in record.members() {
            assert_descriptions(header, member.documentation());
        }
    }
    assert_map_descriptions(header, &registers);
    assert_map_descriptions(header, &control);
    Ok(())
}

#[test]
fn generated_header_matches_current_rust_wire_bytes_and_mmio() -> TestResult {
    // Given: a generated header and a C consumer using actual exported accessors.
    let temp = TempDirectory::new()?;
    let header = ProcessCommand::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3", "--write-dir"])
        .arg(&temp.0)
        .output()?;
    assert_success("header generator", &header);
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/header_c/consumer.c");
    let binary = temp.0.join("consumer");
    let compile = ProcessCommand::new("cc")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .arg("-I")
        .arg(&temp.0)
        .arg(&fixture)
        .arg("-o")
        .arg(&binary)
        .output()?;
    assert_success("strict C11 compilation", &compile);

    // When: the compiled consumer encodes records and reports the exported register offsets.
    let output = ProcessCommand::new(&binary).output()?;

    // Then: C and Rust both match independently specified little-endian bytes and MMIO addresses.
    assert_success("C consumer", &output);
    let dma_bytes = dma_wire::bytes()?;
    let offsets = [
        0_u32, 4, 8, 12, 16, 20, 24, 28, 4096, 4104, 4108, 4112, 4120, 4128, 4132, 4136, 4140,
        4144, 4148, 4152, 4156, 4160, 4164, 4168, 4172, 8192, 8200, 8204, 8208, 8216, 8224, 8228,
        8232, 8236, 8240, 8244, 8248, 8252, 8256, 8260, 8264, 8268, 12288, 12296, 12300, 12304,
        12312, 12320, 12324, 12328, 12332, 12336, 12340, 12344, 12348, 12352, 12356, 12360, 12364,
    ];
    let resolved = DmaRegisters::new(3)?.resolve()?;
    let rust_offsets = resolved
        .registers()
        .iter()
        .map(hw_model_schema::ResolvedRegister::offset)
        .collect::<Vec<_>>();
    assert_eq!(
        rust_offsets,
        offsets.map(|offset| usize::try_from(offset).unwrap())
    );
    let control_offsets = [0_u8, 8, 12, 16, 20, 24, 28, 32];
    assert_eq!(
        ControlRegisters::resolve()?
            .registers()
            .iter()
            .map(hw_model_schema::ResolvedRegister::offset)
            .collect::<Vec<_>>(),
        control_offsets.map(usize::from)
    );
    let offset_bytes: Vec<_> = offsets.into_iter().flat_map(u32::to_le_bytes).collect();
    let expected = [
        dma_bytes.as_slice(),
        offset_bytes.as_slice(),
        &CONTROL_BYTES.concat(),
        control_offsets.as_slice(),
        QUEUE_BYTES.as_slice(),
    ]
    .concat();
    assert_eq!(output.stdout, expected);
    Ok(())
}

fn assert_map_descriptions<I: Copy>(header: &str, registers: &ResolvedRegisterMap<I>) {
    assert_descriptions(header, registers.documentation());
    for placement in registers.placements() {
        assert_descriptions(header, placement.documentation());
    }
    for register in registers.registers() {
        assert_descriptions(header, register.documentation());
        for field in register.definition().fields() {
            assert_descriptions(header, field.documentation());
        }
    }
}

fn assert_descriptions(header: &str, documentation: Documentation) {
    for text in [documentation.short_description, documentation.description]
        .into_iter()
        .flatten()
    {
        for line in text.lines() {
            assert!(
                header.contains(line),
                "missing schema documentation: {line}"
            );
        }
    }
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pdma-contract-header-c-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "failed to clean C header test directory {}: {error}",
                self.0.display()
            );
        }
    }
}
