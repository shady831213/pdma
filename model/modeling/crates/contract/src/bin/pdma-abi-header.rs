//! Generates or verifies the PDMA ABI-v1 C data header.

use std::{
    env,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
};

use pdma_model_contract::abi::header::render_c_header;

struct TemporaryOutput {
    path: PathBuf,
    active: bool,
}

impl TemporaryOutput {
    const fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(mut self) {
        self.active = false;
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

enum Command {
    Check(PathBuf),
    Write(PathBuf),
}

#[derive(Debug)]
enum HeaderError {
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    CreateTemporary {
        path: PathBuf,
        source: io::Error,
    },
    InvalidOutputPath {
        path: PathBuf,
    },
    Mismatch {
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
    Sync {
        path: PathBuf,
        source: io::Error,
    },
    Usage,
    Write {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, source } => {
                write!(formatter, "cannot create {}: {source}", path.display())
            }
            Self::CreateTemporary { path, source } => {
                write!(
                    formatter,
                    "cannot create temporary {}: {source}",
                    path.display()
                )
            }
            Self::InvalidOutputPath { path } => {
                write!(formatter, "invalid output path: {}", path.display())
            }
            Self::Mismatch { path } => {
                write!(formatter, "generated header differs: {}", path.display())
            }
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Rename { from, to, source } => write!(
                formatter,
                "cannot replace {} with {}: {source}",
                to.display(),
                from.display()
            ),
            Self::Sync { path, source } => {
                write!(formatter, "cannot sync {}: {source}", path.display())
            }
            Self::Usage => write!(
                formatter,
                "usage: pdma-abi-header (--write PATH | --check PATH)"
            ),
            Self::Write { path, source } => {
                write!(formatter, "cannot write {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for HeaderError {}

fn main() {
    if let Err(error) = run(env::args_os().skip(1)) {
        eprintln!("pdma-abi-header: {error}");
        process::exit(1);
    }
}

fn run(arguments: impl Iterator<Item = OsString>) -> Result<(), HeaderError> {
    let command = parse_command(arguments)?;
    let rendered = render_c_header();
    match command {
        Command::Check(path) => check(&path, &rendered),
        Command::Write(path) => write_atomically(&path, rendered.as_bytes()),
    }
}

fn parse_command(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, HeaderError> {
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(option), Some(path), None) if option == "--check" => {
            Ok(Command::Check(PathBuf::from(path)))
        }
        (Some(option), Some(path), None) if option == "--write" => {
            Ok(Command::Write(PathBuf::from(path)))
        }
        _ => Err(HeaderError::Usage),
    }
}

fn check(path: &Path, rendered: &str) -> Result<(), HeaderError> {
    let actual = fs::read(path).map_err(|source| HeaderError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if actual == rendered.as_bytes() {
        Ok(())
    } else {
        Err(HeaderError::Mismatch {
            path: path.to_path_buf(),
        })
    }
}

fn write_atomically(path: &Path, rendered: &[u8]) -> Result<(), HeaderError> {
    let parent = path
        .parent()
        .ok_or_else(|| HeaderError::InvalidOutputPath {
            path: path.to_path_buf(),
        })?;
    fs::create_dir_all(parent).map_err(|source| HeaderError::CreateDirectory {
        path: parent.to_path_buf(),
        source,
    })?;
    let temporary = TemporaryOutput::new(temporary_path(path)?);
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(temporary.path())
        .map_err(|source| HeaderError::CreateTemporary {
            path: temporary.path().to_path_buf(),
            source,
        })?;
    write_and_sync(file, temporary.path(), rendered)?;
    fs::rename(temporary.path(), path).map_err(|source| HeaderError::Rename {
        from: temporary.path().to_path_buf(),
        to: path.to_path_buf(),
        source,
    })?;
    temporary.disarm();
    Ok(())
}

fn temporary_path(path: &Path) -> Result<PathBuf, HeaderError> {
    let parent = path
        .parent()
        .ok_or_else(|| HeaderError::InvalidOutputPath {
            path: path.to_path_buf(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| HeaderError::InvalidOutputPath {
            path: path.to_path_buf(),
        })?;
    Ok(parent.join(format!(".{}.{}.tmp", name.to_string_lossy(), process::id())))
}

fn write_and_sync(mut file: File, temporary: &Path, rendered: &[u8]) -> Result<(), HeaderError> {
    file.write_all(rendered)
        .map_err(|source| HeaderError::Write {
            path: temporary.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| HeaderError::Sync {
        path: temporary.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{Command, HeaderError, parse_command};
    use std::ffi::OsString;

    #[test]
    fn rejects_malformed_arguments() {
        // Given: incomplete and over-specified generator command lines.
        let incomplete = [OsString::from("--check")];
        let extra = [
            OsString::from("--write"),
            OsString::from("header.h"),
            OsString::from("extra"),
        ];

        // When: the CLI parses each argument list.
        let incomplete_result = parse_command(incomplete.into_iter());
        let extra_result = parse_command(extra.into_iter());

        // Then: neither malformed command can select a write or check operation.
        assert!(matches!(incomplete_result, Err(HeaderError::Usage)));
        assert!(matches!(extra_result, Err(HeaderError::Usage)));
    }

    #[test]
    fn accepts_one_explicit_check_path() {
        // Given: exactly one supported non-mutating command.
        let arguments = [OsString::from("--check"), OsString::from("header.h")];

        // When: the CLI parses the command.
        let command = parse_command(arguments.into_iter());

        // Then: it selects check mode for the supplied path.
        assert!(
            matches!(command, Ok(Command::Check(path)) if path == std::path::Path::new("header.h"))
        );
    }
}
