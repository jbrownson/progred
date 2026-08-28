//! The compile service: plugin Rust in, wasm out, over pipes — source
//! rides stdin, the module rides stdout, diagnostics ride stderr as
//! rustc's JSON. No files on our side, no cargo in the loop
//! (`../../docs/projections.md`).

use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// The toolchain is a pinned runtime component, resolved through
/// rustup by name — never bare `rustc`, whose PATH winner may lack
/// the wasm target (Homebrew's does).
const TOOLCHAIN: &str = "stable";

// Message and span await the squiggle round: display shows `rendered`
// today, structure will consume the offsets.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    /// The primary span, as byte offsets into the submitted source.
    pub span: Option<(usize, usize)>,
    /// rustc's own human rendering, kept whole for display.
    pub rendered: String,
}

#[derive(Debug)]
pub enum CompileError {
    /// The toolchain itself failed to run.
    Toolchain(String),
    /// rustc refused the source.
    Source(Vec<Diagnostic>),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Toolchain(message) => write!(f, "{message}"),
            CompileError::Source(diagnostics) => {
                for diagnostic in diagnostics {
                    write!(f, "{}", diagnostic.rendered)?;
                }
                Ok(())
            }
        }
    }
}

/// The service's own failures — spawning, piping, joining — as
/// distinct from rustc refusing the source.
fn service_error(error: impl std::fmt::Display) -> CompileError {
    CompileError::Toolchain(format!("compile service: {error}"))
}

fn rustc_command() -> Command {
    std::env::var_os("RUSTC").map_or_else(
        || {
            let mut command = Command::new("rustup");
            command.args(["run", TOOLCHAIN, "rustc"]);
            command
        },
        Command::new,
    )
}

pub fn compile(source: &str) -> Result<Vec<u8>, CompileError> {
    // rustc stages stdout output as `stdout.<crate>` in its cwd, so
    // concurrent compiles sharing a directory clobber each other:
    // every call gets its own scratch, which is also the child's
    // TMPDIR so intermediates follow. Under the system temp — the one
    // writable root an installed app is guaranteed (a bundle is
    // read-only and signed), and `temp_dir` honors $TMPDIR, the knob
    // sandboxes redirect.
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "progred-compile-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&scratch)
        .map_err(|error| service_error(format!("creating scratch: {error}")))?;
    let result = run(source, &scratch);
    std::fs::remove_dir_all(&scratch).ok();
    result
}

fn run(source: &str, scratch: &std::path::Path) -> Result<Vec<u8>, CompileError> {
    let mut child = rustc_command()
        .args([
            "-",
            "--edition",
            "2024",
            "--target",
            "wasm32-unknown-unknown",
            "--crate-type",
            "cdylib",
            "-O",
            // DWARF is ~30× the module and nothing in the host reads
            // it; names stay for backtraces and wasm dumps. A future
            // debug flavor recompiles unstripped in one call.
            "-C",
            "strip=debuginfo",
            "--error-format",
            "json",
            "-o",
            "-",
        ])
        .current_dir(scratch)
        .env("TMPDIR", scratch)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| service_error(format!("spawning rustup: {error}")))?;
    let mut stdin = child.stdin.take().expect("piped");
    stdin
        .write_all(source.as_bytes())
        .map_err(|error| service_error(format!("feeding source: {error}")))?;
    drop(stdin);
    // Stderr drains on its own thread so neither pipe can fill and
    // wedge the other.
    let mut stderr = child.stderr.take().expect("piped");
    let drain = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).map(|_| text)
    });
    let mut wasm = Vec::new();
    child
        .stdout
        .take()
        .expect("piped")
        .read_to_end(&mut wasm)
        .map_err(|error| service_error(format!("reading module: {error}")))?;
    let status = child
        .wait()
        .map_err(|error| service_error(format!("waiting on rustc: {error}")))?;
    let stderr = drain
        .join()
        .map_err(|_| service_error("stderr reader panicked"))?
        .map_err(|error| service_error(format!("reading diagnostics: {error}")))?;
    if status.success() {
        Ok(wasm)
    } else {
        Err(CompileError::Source(diagnostics(&stderr)))
    }
}

/// One JSON object per stderr line; errors keep their message, their
/// primary span, and rustc's rendering. The "aborting due to…"
/// summary restates the count and is dropped.
fn diagnostics(stderr: &str) -> Vec<Diagnostic> {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|json| json["$message_type"] == "diagnostic" && json["level"] == "error")
        .filter(|json| {
            !json["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("aborting due to"))
        })
        .map(|json| Diagnostic {
            message: json["message"].as_str().unwrap_or_default().to_string(),
            span: json["spans"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|span| span["is_primary"] == true)
                .and_then(|span| {
                    Some((
                        span["byte_start"].as_u64()? as usize,
                        span["byte_end"].as_u64()? as usize,
                    ))
                }),
            rendered: json["rendered"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_becomes_wasm() {
        let wasm =
            compile("#[unsafe(no_mangle)]\npub extern \"C\" fn answer() -> u32 { 42 }").unwrap();
        assert_eq!(&wasm[..4], b"\0asm");
    }

    #[test]
    fn refusals_carry_spans_into_the_source() {
        let source = "pub fn broken() -> f64 { missing }";
        let error = compile(source).unwrap_err();
        let CompileError::Source(diagnostics) = error else {
            panic!("expected source diagnostics, got {error}");
        };
        let primary = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.span.is_some())
            .unwrap();
        assert!(primary.message.contains("missing"));
        let (start, end) = primary.span.unwrap();
        assert_eq!(&source[start..end], "missing");
    }
}
