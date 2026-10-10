use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use foldshift_core::{FcError, Inspection, MediaKind, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const SUPPORTED_EXTENSIONS: &[&str] = &["pdf"];

/// Options controlling PDF merge operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MergeOptions {
    /// Overwrite existing destination file if present.
    pub force: bool,
}

/// Options controlling PDF page extraction operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractOptions {
    /// Overwrite existing destination file if present.
    pub force: bool,
    /// Page range specification (e.g. "1", "1-3", "1,5,7", "1-z").
    pub pages: String,
}

/// Checks whether a given file extension is supported as PDF.
pub fn is_supported_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&lower.as_str())
}

/// Validates that a file starts with a valid PDF header (`%PDF-`) and extracts its version.
pub fn validate_pdf_header(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FcError::InputMissing(path.to_path_buf())
        } else {
            FcError::Io(e)
        }
    })?;

    let mut buf = [0u8; 1024];
    let bytes_read = file.read(&mut buf).map_err(FcError::Io)?;
    if bytes_read < 4 {
        return Err(FcError::UnsupportedContent(
            "file is too small to be a valid PDF document".into(),
        ));
    }

    let header_chunk = &buf[..bytes_read];
    // Per PDF specification ISO 32000-1, %PDF- must appear in the first 1024 bytes
    let header_str = String::from_utf8_lossy(header_chunk);
    if let Some(pos) = header_str.find("%PDF-") {
        let rest = &header_str[pos + 5..];
        let version: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if !version.is_empty() {
            return Ok(version);
        }
        return Ok("unknown".into());
    }

    Err(FcError::UnsupportedContent(
        "file is not a valid PDF document (missing %PDF- header)".into(),
    ))
}

/// Validates page range specification syntax (1-indexed, positive numbers, valid delimiters).
pub fn validate_page_range(spec: &str) -> Result<()> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(FcError::InvalidOption(
            "page range specification cannot be empty".into(),
        ));
    }

    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(FcError::InvalidOption(
                "page range contains an empty segment".into(),
            ));
        }

        if let Some((start_s, end_s)) = part.split_once('-') {
            let start_s = start_s.trim();
            let end_s = end_s.trim();

            if start_s.is_empty() || end_s.is_empty() {
                return Err(FcError::InvalidOption(format!(
                    "invalid page range '{}': range endpoints cannot be empty",
                    part
                )));
            }

            if start_s == "z" {
                return Err(FcError::InvalidOption(format!(
                    "invalid range start in '{}': 'z' can only be range end",
                    part
                )));
            }

            let start = start_s.parse::<u32>().map_err(|_| {
                FcError::InvalidOption(format!("invalid range start '{}' in '{}'", start_s, part))
            })?;

            if start == 0 {
                return Err(FcError::InvalidOption(format!(
                    "invalid page number '{}' in '{}': pages are 1-indexed",
                    start, part
                )));
            }

            if end_s != "z" {
                let end = end_s.parse::<u32>().map_err(|_| {
                    FcError::InvalidOption(format!("invalid range end '{}' in '{}'", end_s, part))
                })?;

                if end == 0 {
                    return Err(FcError::InvalidOption(format!(
                        "invalid page number '{}' in '{}': pages are 1-indexed",
                        end, part
                    )));
                }
            }
        } else if part != "z" {
            let page = part.parse::<u32>().map_err(|_| {
                FcError::InvalidOption(format!(
                    "invalid page specification '{}': must be a positive integer or 'z'",
                    part
                ))
            })?;
            if page == 0 {
                return Err(FcError::InvalidOption(format!(
                    "invalid page number '{}': pages are 1-indexed",
                    page
                )));
            }
        }
    }
    Ok(())
}

/// Resolves the executable path of the qpdf engine.
///
/// Priority:
/// 1. `FOLDSHIFT_QPDF_PATH` environment variable.
/// 2. Installed and healthy `pdf` pack in the FoldShift packs directory.
/// 3. Host system `PATH` if `qpdf` is installed globally.
pub fn resolve_qpdf_engine() -> Result<PathBuf> {
    if let Ok(path_str) = std::env::var("FOLDSHIFT_QPDF_PATH") {
        let p = PathBuf::from(path_str.trim());
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Some(pack) = foldshift_core::pack::get_installed_engine("pdf") {
        let entrypoint = pack.resolved_entrypoint();
        if entrypoint.is_file() {
            return Ok(entrypoint);
        }
    }

    // Check system PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let bin_name = if cfg!(windows) { "qpdf.exe" } else { "qpdf" };
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(FcError::CapabilityUnavailable(
        "pdf capability is not installed. Run 'foldshift packs list' to check engine packs, or install the qpdf pack."
            .into(),
    ))
}

/// Executes a qpdf subprocess with bounded execution limits, pipe deadlock prevention,
/// and clean error handling.
pub fn run_qpdf(engine: &Path, args: &[String]) -> Result<String> {
    let mut cmd = Command::new(engine);
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        FcError::Conversion(format!(
            "failed to spawn qpdf engine at '{}': {}",
            engine.display(),
            e
        ))
    })?;

    // Drain stdout and stderr concurrently to prevent pipe buffer deadlocks
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    const MAX_OUTPUT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB output safety cap

    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stdout_pipe {
            let mut limited = pipe.take(MAX_OUTPUT_BYTES);
            let _ = limited.read_to_end(&mut buf);
        }
        buf
    });

    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stderr_pipe {
            let mut limited = pipe.take(MAX_OUTPUT_BYTES);
            let _ = limited.read_to_end(&mut buf);
        }
        buf
    });

    // Wait for child process with 60s timeout guard
    let timeout = Duration::from_secs(60);
    let start = std::time::Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(FcError::Conversion(format!(
                        "qpdf engine timed out after {}s",
                        timeout.as_secs()
                    )));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(FcError::Io(e));
            }
        }
    };

    let stdout_bytes = stdout_reader.join().unwrap_or_default();
    let stderr_bytes = stderr_reader.join().unwrap_or_default();

    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

    if status.success() {
        return Ok(stdout);
    }

    // Handle known error conditions
    let stderr_trimmed = stderr.trim();
    if stderr_trimmed.contains("password") {
        return Err(FcError::Conversion(
            "PDF document is password-protected or encrypted".into(),
        ));
    }
    Err(FcError::Conversion(format!(
        "qpdf engine failed (exit code {:?}): {}",
        status.code(),
        stderr_trimmed
    )))
}

/// Inspects a PDF file's page count, version, and properties.
pub fn inspect(path: &Path) -> Result<Inspection> {
    if !path.exists() {
        return Err(FcError::InputMissing(path.to_path_buf()));
    }
    if !path.is_file() {
        return Err(FcError::InputMissing(path.to_path_buf()));
    }

    let pdf_version = validate_pdf_header(path)?;
    let metadata = std::fs::metadata(path).map_err(FcError::Io)?;
    let file_bytes = metadata.len();

    let engine = resolve_qpdf_engine()?;
    let stdout = run_qpdf(
        &engine,
        &[
            "--show-npages".to_string(),
            path.to_string_lossy().to_string(),
        ],
    )?;

    let page_count = stdout.trim().parse::<u32>().unwrap_or(1);

    let inspection = Inspection::new(path, "PDF", MediaKind::Pdf, file_bytes)
        .with_property("page_count", json!(page_count))
        .with_property("pdf_version", json!(pdf_version));

    Ok(inspection)
}

/// Merges multiple PDF files in order into a single output file.
///
/// Follows the Same-File and Atomic Publication policy:
/// - Fails early if destination exists and `force` is false.
/// - Writes to an isolated temporary file in destination directory first.
/// - Original inputs and destinations remain intact if qpdf fails.
/// - Atomically replaces destination only upon 100% success.
pub fn merge(inputs: &[PathBuf], output: &Path, options: MergeOptions) -> Result<()> {
    if inputs.is_empty() {
        return Err(FcError::InvalidOption(
            "at least one input PDF file is required for merge".into(),
        ));
    }

    // Preflight check all inputs
    for input in inputs {
        if !input.exists() || !input.is_file() {
            return Err(FcError::InputMissing(input.clone()));
        }
        validate_pdf_header(input)?;
    }

    // Same-file / destination preflight
    if output.exists() && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }

    let engine = resolve_qpdf_engine()?;

    let parent_dir = output.parent().unwrap_or_else(|| Path::new("."));
    if !parent_dir.exists() {
        std::fs::create_dir_all(parent_dir).map_err(FcError::Io)?;
    }

    // Create temporary output file in destination directory for atomic publication
    let temp_file = tempfile::Builder::new()
        .prefix(".foldshift_pdf_merge_")
        .suffix(".tmp")
        .tempfile_in(parent_dir)
        .map_err(FcError::Io)?;

    let temp_path = temp_file.path().to_path_buf();

    // Prepare arguments: qpdf --empty --pages in1 in2 ... -- temp_output
    let mut args = vec!["--empty".to_string(), "--pages".to_string()];
    for input in inputs {
        args.push(input.to_string_lossy().to_string());
    }
    args.push("--".to_string());
    args.push(temp_path.to_string_lossy().to_string());

    // Execute qpdf
    run_qpdf(&engine, &args)?;

    // Atomically persist over target
    temp_file
        .persist(output)
        .map_err(|e| FcError::Io(e.error))?;

    Ok(())
}

/// Extracts specified page ranges from a PDF document into an output file.
///
/// Follows the Same-File and Atomic Publication policy:
/// - Fails early on invalid page range or missing file.
/// - Writes to an isolated temporary file in destination directory first.
/// - Original file is preserved even if output targets the same file with `--force`.
/// - Atomically replaces destination only upon 100% success.
pub fn extract(input: &Path, output: &Path, options: ExtractOptions) -> Result<()> {
    if !input.exists() || !input.is_file() {
        return Err(FcError::InputMissing(input.to_path_buf()));
    }

    validate_pdf_header(input)?;
    validate_page_range(&options.pages)?;

    if output.exists() && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }

    let engine = resolve_qpdf_engine()?;

    let parent_dir = output.parent().unwrap_or_else(|| Path::new("."));
    if !parent_dir.exists() {
        std::fs::create_dir_all(parent_dir).map_err(FcError::Io)?;
    }

    let temp_file = tempfile::Builder::new()
        .prefix(".foldshift_pdf_extract_")
        .suffix(".tmp")
        .tempfile_in(parent_dir)
        .map_err(FcError::Io)?;

    let temp_path = temp_file.path().to_path_buf();

    // Prepare arguments: qpdf --empty --pages input pages_spec -- temp_output
    let args = vec![
        "--empty".to_string(),
        "--pages".to_string(),
        input.to_string_lossy().to_string(),
        options.pages,
        "--".to_string(),
        temp_path.to_string_lossy().to_string(),
    ];

    run_qpdf(&engine, &args)?;

    temp_file
        .persist(output)
        .map_err(|e| FcError::Io(e.error))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pdf(pages: u32) -> Vec<u8> {
        format!(
            "%PDF-1.7\n\
            1 0 obj\n\
            << /Type /Catalog /Pages 2 0 R >>\n\
            endobj\n\
            2 0 obj\n\
            << /Type /Pages /Count {} >>\n\
            endobj\n\
            xref\n\
            0 3\n\
            0000000000 65535 f \n\
            trailer\n\
            << /Size 3 /Root 1 0 R >>\n\
            startxref\n\
            100\n\
            %%EOF\n",
            pages
        )
        .into_bytes()
    }

    #[test]
    fn validates_pdf_header_and_extracts_version() {
        let dir = tempfile::tempdir().unwrap();
        let pdf_path = dir.path().join("doc.pdf");
        std::fs::write(&pdf_path, create_test_pdf(3)).unwrap();

        let version = validate_pdf_header(&pdf_path).unwrap();
        assert_eq!(version, "1.7");

        let bad_path = dir.path().join("not_a_pdf.txt");
        std::fs::write(&bad_path, b"Hello world, not a pdf!").unwrap();
        let err = validate_pdf_header(&bad_path).unwrap_err();
        assert!(matches!(err, FcError::UnsupportedContent(_)));
    }

    #[test]
    fn page_range_validation_rules() {
        assert!(validate_page_range("1").is_ok());
        assert!(validate_page_range("1-5").is_ok());
        assert!(validate_page_range("1,3,5").is_ok());
        assert!(validate_page_range("1-3,5-7").is_ok());
        assert!(validate_page_range("1-z").is_ok());
        assert!(validate_page_range("z").is_ok());

        assert!(validate_page_range("").is_err());
        assert!(validate_page_range("0").is_err());
        assert!(validate_page_range("0-3").is_err());
        assert!(validate_page_range("1-0").is_err());
        assert!(validate_page_range("-5").is_err());
        assert!(validate_page_range("5-").is_err());
        assert!(validate_page_range("abc").is_err());
        assert!(validate_page_range("1,,2").is_err());
    }

    #[test]
    fn uninstalled_engine_returns_capability_unavailable() {
        // Clear any engine env
        let prev = std::env::var("FOLDSHIFT_QPDF_PATH").ok();
        unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", "non_existent_engine_path_12345") };

        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("test.pdf");
        std::fs::write(&pdf, create_test_pdf(1)).unwrap();

        let err = inspect(&pdf).unwrap_err();
        assert!(matches!(err, FcError::CapabilityUnavailable(_)));

        if let Some(val) = prev {
            unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", val) };
        } else {
            unsafe { std::env::remove_var("FOLDSHIFT_QPDF_PATH") };
        }
    }
}
