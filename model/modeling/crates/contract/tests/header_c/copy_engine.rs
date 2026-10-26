use super::{TempDirectory, TestResult, assert_success};
use std::{path::PathBuf, process::Command};

const SCALAR_LAYOUTS: [&[(usize, u8)]; 8] = [
    &[
        (16, 0x38),
        (17, 0x12),
        (24, 9),
        (28, 1),
        (32, 0x78),
        (33, 0x56),
    ],
    &[
        (16, 0x38),
        (17, 0x12),
        (24, 19),
        (32, 0x78),
        (33, 0x56),
        (40, 9),
        (44, 1),
    ],
    &[
        (16, 0x38),
        (17, 0x12),
        (24, 9),
        (28, 1),
        (32, 0x78),
        (33, 0x56),
        (40, 10),
    ],
    &[
        (16, 0x38),
        (17, 0x12),
        (24, 9),
        (28, 1),
        (32, 3),
        (36, 6),
        (40, 0x78),
        (41, 0x56),
        (48, 0xbc),
        (49, 0x9a),
    ],
    &[(16, 0x38), (17, 0x12), (24, 9), (28, 1), (32, 1)],
    &[
        (16, 0x38),
        (17, 0x12),
        (24, 9),
        (28, 1),
        (32, 0x78),
        (33, 0x56),
    ],
    &[],
    &[(16, 0x38), (17, 0x12), (24, 9)],
];

#[test]
fn scalar_commands_have_identical_strict_c_wire_encoding() -> TestResult {
    // Given: role headers and a C consumer of every scalar command payload.
    let temp = TempDirectory::new()?;
    let generated = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3", "--contexts", "3", "--write-dir"])
        .arg(&temp.0)
        .output()?;
    assert_success("header generator", &generated);
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/header_c/copy_engine.c");
    let binary = temp.0.join("copy_engine");
    let compiled = Command::new("cc")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .arg("-I")
        .arg(&temp.0)
        .arg(fixture)
        .arg("-o")
        .arg(&binary)
        .output()?;
    assert_success("strict scalar C compilation", &compiled);
    // When: the C program encodes every scalar family using generated accessors.
    let output = Command::new(binary).output()?;
    // Then: all eight records match independent byte layouts, including reserved bytes.
    assert_success("scalar C consumer", &output);
    let mut expected = Vec::new();
    for (index, fields) in SCALAR_LAYOUTS.into_iter().enumerate() {
        let mut bytes = [0_u8; 96];
        bytes[8] = 42;
        bytes[0] = u8::try_from(index + 1)?;
        for &(offset, value) in fields {
            *bytes.get_mut(offset).unwrap() = value;
        }
        expected.extend_from_slice(&bytes);
    }
    for mode in 0_u8..4 {
        let mut bytes = [0_u8; 96];
        bytes[1] = mode;
        bytes[4..32].fill(0xff);
        bytes[32..48].fill(0xff);
        bytes[64..80].fill(0xff);
        if mode & 1 != 0 {
            bytes[48..64].fill(0xff);
        }
        if mode & 2 != 0 {
            bytes[80..96].fill(0xff);
        }
        expected.extend_from_slice(&bytes);
    }
    assert_eq!(output.stdout, expected);
    for bytes in output.stdout.chunks_exact(96) {
        let decoded = pdma_model_contract::dma::Command::from_bytes(bytes)?;
        assert_eq!(decoded.to_bytes()?.as_slice(), bytes);
    }
    Ok(())
}
