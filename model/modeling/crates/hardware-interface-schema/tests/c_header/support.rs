use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    process::{Command, Output},
};

use hardware_interface_schema::export::backends::c_header::CHeaderExportError;
use hardware_interface_schema::export::{Exporter, InMemoryArtifactSink};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const STRICT_C11_FLAGS: [&str; 5] = [
    "-std=c11",
    "-pedantic-errors",
    "-Wall",
    "-Wextra",
    "-Werror",
];

pub fn render<S, E>(
    exporter: &E,
    schema: &S,
) -> TestResult<hardware_interface_schema::export::Artifact>
where
    E: Exporter<S, Error = CHeaderExportError>,
{
    let mut sink = InMemoryArtifactSink::<CHeaderExportError>::new();
    exporter.export(schema, &mut sink)?;
    let mut artifacts = sink.into_artifacts().into_iter();
    let artifact = artifacts
        .next()
        .ok_or_else(|| io::Error::other("exporter emitted no artifact"))?;
    if artifacts.next().is_some() {
        return Err(io::Error::other("exporter emitted multiple artifacts").into());
    }
    Ok(artifact)
}

pub fn compile_fragments(label: &str, fragments: &[&[u8]], main: &[u8]) -> TestResult {
    let temp = TempDirectory::new(label)?;
    let source = temp.path.join("wrapper.c");
    let binary = temp.path.join("wrapper");
    let mut wrapper = b"#include <stdint.h>\n\n".to_vec();
    for fragment in fragments {
        wrapper.extend_from_slice(fragment);
        wrapper.push(b'\n');
    }
    wrapper.extend_from_slice(main);
    fs::write(&source, wrapper)?;
    let compile = Command::new("cc")
        .args(STRICT_C11_FLAGS)
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()?;
    assert_success("C compiler", &compile);
    let run = Command::new(&binary).output()?;
    assert_success("compiled wrapper", &run);
    Ok(())
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(label: &str) -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "hardware-interface-schema-c-header-{label}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub const HAPPY_C_MAIN: &[u8] = br"
int main(void) {
    uint8_t bytes[C_QULABQX2D9QX20PACKET_SIZE] = {0};
    C_QULABQX2D9QX20PACKET_STORE_C_SEQQUENCE(bytes, UINT16_C(0x1234));
    C_QULABQX2D9QX20PACKET_STORE_C_MEASURE(bytes, UINT32_C(0x89abcdef));
    C_QULABQX2D9QX20PACKET_STORE_C_TIMESTAMP(bytes, UINT64_C(0x0102030405060708));
    if (C_QULABQX2D9QX20PACKET_LOAD_C_SEQQUENCE(bytes) != UINT16_C(0x1234)) return 1;
    if (C_QULABQX2D9QX20PACKET_LOAD_C_MEASURE(bytes) != UINT32_C(0x89abcdef)) return 2;
    if (C_QULABQX2D9QX20PACKET_LOAD_C_TIMESTAMP(bytes) != UINT64_C(0x0102030405060708)) return 3;
    if (bytes[2] != UINT8_C(0x12) || bytes[3] != UINT8_C(0x34)) return 4;
    if (bytes[5] != UINT8_C(0xef) || bytes[8] != UINT8_C(0x89)) return 5;
    if (C_QULABQX2D9QX20MAP_C_CHANNELS_1_C_LEVEL_OFFSET != UINT64_C(0x40)) return 6;
    if (C_QULABQX2D9QX20MAP_C_CHANNELS_1_C_FLAGS_OFFSET != UINT64_C(0x50)) return 7;
    if (C_QULABQX2D9QX20MAP_C_COMMAND_ACCESS != C_QULABQX2D9QX20MAP_ACCESS_WRITE_ONLY) return 8;
    return 0;
}
";

pub const COLLISION_C_MAIN: &[u8] = br"
int main(void) {
    uint8_t bytes[C_COLLISIONQX20RECORD_SIZE] = {0};
    C_COLLISIONQX20RECORD_STORE_C_AQX2DB(bytes, UINT8_C(7));
    C_COLLISIONQX20RECORD_STORE_C_AQSB(bytes, UINT16_C(0x1234));
    if (C_COLLISIONQX20RECORD_LOAD_C_AQX2DB(bytes) != UINT8_C(7)) return 1;
    if (C_COLLISIONQX20RECORD_LOAD_C_AQSB(bytes) != UINT16_C(0x1234)) return 2;
    if (C_COLLISIONQX20MAP_C_AQX2DB_OFFSET != UINT64_C(0)) return 3;
    if (C_COLLISIONQX20MAP_C_AQSB_OFFSET != UINT64_C(4)) return 4;
    if (C_COLLISIONQX20MAP_C_BANKQX2DA_BASE != UINT64_C(32)) return 5;
    if (C_COLLISIONQX20MAP_C_BANKQSA_BASE != UINT64_C(64)) return 6;
    if (C_COLLISIONQX20MAP_C_BANKQX2DA_0_C_AQX2DB_OFFSET != UINT64_C(32)) return 7;
    if (C_COLLISIONQX20MAP_C_BANKQX2DA_0_C_AQSB_OFFSET != UINT64_C(33)) return 8;
    if (C_COLLISIONQX20MAP_C_BANKQSA_0_C_AQX2DB_OFFSET != UINT64_C(64)) return 9;
    if (C_COLLISIONQX20MAP_C_BANKQSA_0_C_AQSB_OFFSET != UINT64_C(65)) return 10;
    return 0;
}
";

pub const PREFIX_C_MAIN: &[u8] = b"int main(void) { return 0; }\n";

pub const NESTED_C_MAIN: &[u8] = br"
int main(void) {
    if (C_NESTED_C_ITEMSQX5B0QX5DQX2EVALUE_OFFSET != UINT64_C(4)) return 1;
    if (C_NESTED_C_ITEMSQX5B1QX5DQX2EVALUE_OFFSET != UINT64_C(8)) return 2;
    return 0;
}
";
