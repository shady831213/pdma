use super::*;
use hw_model_schema::analysis::AnalyzedSchema;
use hw_model_schema::{
    RegisterWindow,
    export::{InMemoryArtifactSink, backends::c_header::CRegisterBackend, generate},
};
use std::{fmt::Write as _, process::Command};

#[test]
fn strict_c11_retains_nested_singleton_topology_when_members_are_hidden()
-> Result<(), Box<dyn std::error::Error>> {
    let layout = Nested::new(3, 1)?;
    let resolved = layout.resolve()?.project_access(|register| {
        (register.id() == NestedRegisterId::Tail).then_some(hw_model_schema::Access::ReadOnly)
    })?;
    let mut sink = InMemoryArtifactSink::new();
    generate(
        &AnalyzedSchema::register_map("nested.h", &resolved)?,
        CRegisterBackend::new().with_name("nested.h", "nested"),
        &mut sink,
    )?;
    let directory = std::env::var_os("SCHEMA_BANK_EVIDENCE_DIR")
        .map_or_else(
            || std::env::temp_dir().join(format!("schema-bank-nested-c11-{}", std::process::id())),
            std::path::PathBuf::from,
        )
        .join("nested");
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join("nested.h"),
        &sink.artifacts().first().unwrap().contents,
    )?;
    std::fs::write(directory.join("consumer.c"), b"#include <stdint.h>\n#include \"nested.h\"\n_Static_assert(C_NESTED_C_GROUPS_COUNT == 3, \"outer count\");\n_Static_assert(C_NESTED_C_LANES__3_COUNT == 1, \"inner count\");\n_Static_assert(C_NESTED_C_LANES__3_BASE == 32, \"nested base\");\n_Static_assert(C_NESTED_C_LANES__3_PARENT_INDEX == C_NESTED_C_GROUPS_INDEX, \"parent bank\");\n_Static_assert(C_NESTED_C_LANES__3_PARENT_INSTANCE == 2, \"parent instance\");\n_Static_assert(C_NESTED_C_GROUPS_STRIDE == 16, \"occupied stride\");\nint main(void) { return C_NESTED_SIZE != 64; }\n")?;
    let binary = directory.join("consumer");
    let compile = Command::new("cc")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .arg(directory.join("consumer.c"))
        .arg("-o")
        .arg(&binary)
        .output()?;
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(Command::new(binary).status()?.success());
    if std::env::var_os("SCHEMA_BANK_EVIDENCE_DIR").is_none() {
        std::fs::remove_dir_all(directory)?;
    }
    Ok(())
}

#[test]
fn strict_c11_addresses_match_routing_for_runtime_counts() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = std::env::var_os("SCHEMA_BANK_EVIDENCE_DIR").map_or_else(
        || std::env::temp_dir().join(format!("schema-bank-c11-{}", std::process::id())),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(&directory)?;
    for count in [1, 3, 5] {
        let layout = super::contiguous::Grouped::new(count)?;
        let resolved = layout.resolve()?;
        let mut sink = InMemoryArtifactSink::new();
        generate(
            &AnalyzedSchema::register_map("banks.h", &resolved)?,
            CRegisterBackend::new().with_name("banks.h", "banks"),
            &mut sink,
        )?;
        let header = &sink.artifacts().first().unwrap().contents;
        let mut consumer = String::from(
            "#include <stdint.h>\n#include <stdio.h>\n#include \"banks.h\"\nint main(void) {\n",
        );
        for index in 0..count {
            let suffix = if count == 1 {
                String::new()
            } else {
                format!("__{}", index + 1)
            };
            let symbol = format!("C_BANKS_C_QUSQUTQUAQUTQUUQUS{suffix}_OFFSET");
            writeln!(
                consumer,
                "printf(\"%llu\\n\", (unsigned long long){symbol});"
            )?;
        }
        consumer.push_str("return 0;\n}\n");
        let run_dir = directory.join(count.to_string());
        std::fs::create_dir_all(&run_dir)?;
        std::fs::write(run_dir.join("banks.h"), header)?;
        std::fs::write(run_dir.join("consumer.c"), consumer)?;
        let binary = run_dir.join("consumer");
        let compile = Command::new("cc")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
            ])
            .arg(run_dir.join("consumer.c"))
            .arg("-o")
            .arg(&binary)
            .output()?;
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&binary).output()?;
        assert!(run.status.success());
        std::fs::write(run_dir.join("addresses.txt"), &run.stdout)?;
        let addresses = String::from_utf8(run.stdout)?;
        let mut windows: Vec<_> = (0..count)
            .map(|_| RegisterWindow::<super::contiguous::Mab>::new(0))
            .collect();
        for (index, address) in addresses.lines().enumerate() {
            let address = address.parse::<usize>()?;
            assert_eq!(address, 24 + index * 16);
            layout.channels().route(address, |selected, relative| {
                assert_eq!(selected, index);
                windows
                    .get_mut(selected)
                    .unwrap()
                    .write(relative, &37u32.to_le_bytes())
            })?;
            let mut bytes = [0; 4];
            windows.get_mut(index).unwrap().read(8, &mut bytes)?;
            assert_eq!(u32::from_le_bytes(bytes), 37);
        }
    }
    if std::env::var_os("SCHEMA_BANK_EVIDENCE_DIR").is_none() {
        std::fs::remove_dir_all(directory)?;
    }
    Ok(())
}
