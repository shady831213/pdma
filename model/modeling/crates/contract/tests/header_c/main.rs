//! Strict C11 acceptance of the generated current PDMA contract.

#![cfg(feature = "export-c-header")]

mod control_queue;

use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    process::{Command as ProcessCommand, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use hardware_interface_schema::{Documentation, ResolvedRegisterMap};
use pdma_model_contract::{
    Command, CompletionFlagsValues, ControlRegisters, ControlRequest, ControlRequestOpcode,
    ControlResponse, ControlResponseStatus, DmaRegisters, Endpoint, Response, ResponseStatus,
    Segment,
};

type TestResult = Result<(), Box<dyn Error>>;
const QUEUE_BYTES: [u8; 48] = [
    0, 16, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0,
    0, 4, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];
const CONTROL_BYTES: [[u8; 16]; 4] = [
    [1, 2, 3, 4, 5, 6, 7, 8, 1, 0, 0, 0, 0x40, 0x30, 0x20, 0x10],
    [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
    [1, 2, 3, 4, 5, 6, 7, 8, 3, 0, 0, 0, 0x40, 0x30, 0x20, 0x10],
];

#[test]
fn generated_header_preserves_resolved_contract_descriptions() -> TestResult {
    // Given: the current record and MMIO schemas used by the device.
    let records = [
        Segment::SPEC.resolve()?,
        Endpoint::SPEC.resolve()?,
        Command::SPEC.resolve()?,
        Response::SPEC.resolve()?,
        ControlRequest::SPEC.resolve()?,
        ControlResponse::SPEC.resolve()?,
        pdma_model_contract::ControlRingState::SPEC.resolve()?,
        pdma_model_contract::ControlQueue::SPEC.resolve()?,
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
    let command_bytes = [
        7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0,
        0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
        0, 0, 0, 0,
    ];
    let response_bytes = [
        7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0,
    ];
    let segment_bytes = [0, 3, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0];
    let offsets = [
        0_u32, 4, 8, 12, 16, 20, 24, 4096, 4104, 4108, 4112, 4120, 4128, 4132, 4136, 4140, 4144,
        4148, 4152, 4156, 8192, 8200, 8204, 8208, 8216, 8224, 8228, 8232, 8236, 8240, 8244, 8248,
        8252, 12288, 12296, 12300, 12304, 12312, 12320, 12324, 12328, 12332, 12336, 12340, 12344,
        12348,
    ];
    let command = Command {
        flags: CompletionFlagsValues { ioc: true },
        id: 7,
        source: Endpoint::linear(0x100, 8),
        destination: Endpoint::scatter_gather(0x200, 2),
    };
    let response = Response {
        flags: CompletionFlagsValues { ioc: true },
        id: 7,
        status: ResponseStatus::Success,
        bytes_completed: 8,
    };
    let segment = Segment {
        address: 0x300,
        length: 8,
    };
    assert_eq!(command.to_bytes()?, command_bytes);
    assert_eq!(response.to_bytes()?, response_bytes);
    assert_eq!(segment.to_bytes()?, segment_bytes);
    let resolved = DmaRegisters::new(3)?.resolve()?;
    let rust_offsets = resolved
        .registers()
        .iter()
        .map(hardware_interface_schema::ResolvedRegister::offset)
        .collect::<Vec<_>>();
    assert_eq!(
        rust_offsets,
        offsets.map(|offset| usize::try_from(offset).unwrap())
    );
    let control_offsets = [0_u8, 8, 12, 16];
    assert_eq!(
        ControlRegisters::resolve()?
            .registers()
            .iter()
            .map(hardware_interface_schema::ResolvedRegister::offset)
            .collect::<Vec<_>>(),
        control_offsets.map(usize::from)
    );
    let offset_bytes: Vec<_> = offsets.into_iter().flat_map(u32::to_le_bytes).collect();
    let expected = [
        command_bytes.as_slice(),
        response_bytes.as_slice(),
        segment_bytes.as_slice(),
        offset_bytes.as_slice(),
        CONTROL_BYTES.as_flattened(),
        control_offsets.as_slice(),
        QUEUE_BYTES.as_slice(),
    ]
    .concat();
    assert_eq!(output.stdout, expected);
    Ok(())
}

#[test]
fn rust_control_records_match_c_consumer_fixtures() -> TestResult {
    // Given: independent wire bytes for acquisition, release and their responses.
    // When: the Rust contract encodes the same records used by the C consumer.
    let control_records = [
        ControlRequest {
            id: 0x0807_0605_0403_0201,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0x1020_3040,
        }
        .to_bytes()?,
        ControlRequest {
            id: 9,
            opcode: ControlRequestOpcode::AcquireChannel,
            channel: 0,
        }
        .to_bytes()?,
        ControlResponse {
            id: 9,
            status: ControlResponseStatus::Success,
            channel: 2,
        }
        .to_bytes()?,
        ControlResponse {
            id: 0x0807_0605_0403_0201,
            status: ControlResponseStatus::NotAllocated,
            channel: 0x1020_3040,
        }
        .to_bytes()?,
    ];
    // Then: Rust matches the independently specified C acceptance fixtures.
    assert_eq!(control_records, CONTROL_BYTES);
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
