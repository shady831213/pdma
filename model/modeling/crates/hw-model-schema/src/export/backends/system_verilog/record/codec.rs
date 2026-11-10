use crate::Endian;
use std::io::{Error, Write};

pub(super) fn accessors(
    record: &str,
    name: &str,
    bytes: usize,
    endian: Endian,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let constant = format!("{record}_{name}").to_ascii_uppercase();
    let width = bytes * 8;
    writeln!(
        writer,
        "  function automatic logic [{}:0] {record}_get_{name}(input {record}_t record_value);",
        width - 1
    )?;
    for index in 0..bytes {
        let target = match endian {
            Endian::Big => bytes - index - 1,
            Endian::Little => index,
        } * 8;
        writeln!(
            writer,
            "    {record}_get_{name}[{target} +: 8] = record_value[({constant}_OFFSET+{index})*8 +: 8];"
        )?;
    }
    writeln!(writer, "  endfunction")?;
    writeln!(
        writer,
        "  function automatic void {record}_set_{name}(inout {record}_t record_value, input logic [{}:0] value);",
        width - 1
    )?;
    for index in 0..bytes {
        let source = match endian {
            Endian::Big => bytes - index - 1,
            Endian::Little => index,
        } * 8;
        writeln!(
            writer,
            "    record_value[({constant}_OFFSET+{index})*8 +: 8] = value[{source} +: 8];"
        )?;
    }
    writeln!(writer, "  endfunction")
}
