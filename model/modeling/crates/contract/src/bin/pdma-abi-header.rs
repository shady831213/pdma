//! Generates coherent common, Host and firmware C11 headers.

/// Schema-backed assembly of the generated header set.
pub mod header;

use std::{error::Error, ffi::OsStr, fs, io, path::PathBuf};

const HELP: &str = "Usage: pdma-abi-header --channels COUNT [--contexts COUNT] (--write-dir DIR | --check-dir DIR)\n\
    pdma-abi-header --help\n\
    Generates pdma_common.h, pdma_host.h and pdma_firmware.h.\n\
    --write-dir DIR  Replace all three generated headers.\n\
    --check-dir DIR  Fail on missing or stale headers without modifying files.\n";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(OsStr::new("--help")) && args.next().is_none() {
        print!("{HELP}");
        return Ok(());
    }
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, HELP);
    if first.as_deref() != Some(OsStr::new("--channels")) {
        return Err(invalid().into());
    }
    let count = args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .and_then(|arg| arg.parse::<usize>().ok())
        .ok_or_else(invalid)?;
    let mut action = args.next().ok_or_else(invalid)?;
    let contexts = if action == OsStr::new("--contexts") {
        let contexts = args
            .next()
            .and_then(|arg| arg.into_string().ok())
            .and_then(|arg| arg.parse::<usize>().ok())
            .ok_or_else(invalid)?;
        action = args.next().ok_or_else(invalid)?;
        contexts
    } else {
        count
    };
    let check = match action.to_str() {
        Some("--write-dir") => false,
        Some("--check-dir") => true,
        _ => return Err(invalid().into()),
    };
    let directory = PathBuf::from(args.next().ok_or_else(invalid)?);
    if args.next().is_some() {
        return Err(invalid().into());
    }
    let headers = header::generate_with_contexts(count, contexts)?;
    if !check {
        fs::create_dir_all(&directory)?;
    }
    for (name, bytes) in headers {
        let path = directory.join(name);
        if check {
            if fs::read(&path)? != bytes {
                return Err(io::Error::other(format!(
                    "{} is stale; regenerate with --write-dir",
                    path.display()
                ))
                .into());
            }
        } else {
            fs::write(path, bytes)?;
        }
    }
    Ok(())
}
