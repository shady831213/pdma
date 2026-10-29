use hw_model_schema::{Access, Endian, ResolvedRegisterMap};
use std::{collections::BTreeSet, error::Error, fmt::Write};

pub(super) fn map<I: Copy>(
    text: &mut String,
    name: &str,
    map: &ResolvedRegisterMap<I>,
) -> Result<(), Box<dyn Error>> {
    writeln!(text, "#[fmt(skip)]\npub package {name} {{")?;
    writeln!(
        text,
        "    const EXTENT: u32 = {};",
        u32::try_from(map.size())?
    )?;
    let mut names = BTreeSet::new();
    for leaf in map.registers() {
        let path = leaf
            .path()
            .strip_prefix(map.root())
            .and_then(|path| path.strip_prefix('.'))
            .ok_or("invalid leaf path")?;
        let symbol = path.replace('.', "_").to_ascii_uppercase();
        if !names.insert(symbol.clone()) {
            return Err("duplicate RTL leaf name".into());
        }
        let definition = leaf.definition();
        if definition.endian() != Endian::Little || !matches!(definition.width(), 4 | 8) {
            return Err("unsupported DMA register shape".into());
        }
        if let Some(description) = leaf.documentation().short_description {
            for line in description.lines() {
                writeln!(text, "    // {line}")?;
            }
        }
        writeln!(
            text,
            "    const {symbol}_OFFSET: u32 = {};",
            u32::try_from(leaf.offset())?
        )?;
        writeln!(
            text,
            "    const {symbol}_BYTES: u32 = {};",
            definition.width()
        )?;
        writeln!(
            text,
            "    const {symbol}_RESET: u64 = 64'h{:016x};",
            definition.reset_value()
        )?;
        writeln!(
            text,
            "    const {symbol}_WRITABLE: u64 = 64'h{:016x};",
            definition.writable_mask()
        )?;
        let (read, write) = match definition.access() {
            Access::ReadOnly => (true, false),
            Access::WriteOnly => (false, true),
            Access::ReadWrite => (true, true),
        };
        writeln!(text, "    const {symbol}_READ: bbool = {read};")?;
        writeln!(text, "    const {symbol}_WRITE: bbool = {write};")?;
        for field in definition.fields() {
            let field_name = field.name().to_ascii_uppercase();
            writeln!(
                text,
                "    const {symbol}_{field_name}_LSB: u32 = {};",
                field.lsb()
            )?;
            writeln!(
                text,
                "    const {symbol}_{field_name}_BITS: u32 = {};",
                field.bit_width()
            )?;
            if let Some(enumeration) = field.definition().enum_definition() {
                for variant in enumeration.variants() {
                    writeln!(
                        text,
                        "    const {symbol}_{field_name}_{}: u64 = {};",
                        variant.name().to_ascii_uppercase(),
                        variant.value()
                    )?;
                }
            }
        }
    }
    writeln!(text, "}}\n")?;
    Ok(())
}
