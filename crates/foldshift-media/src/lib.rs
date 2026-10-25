use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use foldshift_core::{FcError, Inspection, MediaKind, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma", "opus"];

pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] =
    &["mp4", "mkv", "webm", "mov", "avi", "wmv", "flv", "m4v"];

/// Options controlling audio conversion operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AudioConvertOptions {
    /// Target audio bitrate (e.g. "128k", "192k", "320k").
    pub bitrate: Option<String>,
    /// Target audio sample rate in Hz (e.g. 44100, 48000).
    pub sample_rate: Option<u32>,
    /// Target audio channels count (1 for mono, 2 for stereo).
    pub channels: Option<u8>,
    /// Replace an existing output file.
    pub force: bool,
}

/// Options controlling video conversion and transcoding operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VideoConvertOptions {
    /// Target resolution width x height in pixels, e.g. (1920, 1080).
    pub resolution: Option<(u32, u32)>,
    /// Target width in pixels (proportional scaling if height is omitted).
    pub width: Option<u32>,
    /// Target height in pixels (proportional scaling if width is omitted).
    pub height: Option<u32>,
    /// Constant Rate Factor (CRF) for video quality (e.g. 18 to 28, lower means higher quality).
    pub crf: Option<u8>,
    /// Target video bitrate (e.g. "2M", "5M").
    pub video_bitrate: Option<String>,
    /// Target audio bitrate (e.g. "128k", "192k").
    pub audio_bitrate: Option<String>,
    /// Strip audio stream from output video.
    pub mute_audio: bool,
    /// Target video codec (e.g. "libx264", "libvpx-vp9").
    pub video_codec: Option<String>,
    /// Target framerate in frames per second (e.g. 30.0, 60.0).
    pub fps: Option<f32>,
    /// Replace an existing output file.
    pub force: bool,
}

/// Checks whether a given file extension is a supported audio format.
pub fn is_supported_audio_extension(ext: &str) -> bool {
    let lower = ext.trim_start_matches('.').to_ascii_lowercase();
    SUPPORTED_AUDIO_EXTENSIONS.contains(&lower.as_str())
}

/// Checks whether a given file extension is a supported video format.
pub fn is_supported_video_extension(ext: &str) -> bool {
    let lower = ext.trim_start_matches('.').to_ascii_lowercase();
    SUPPORTED_VIDEO_EXTENSIONS.contains(&lower.as_str())
}

/// Resolves the executable path of the media engine (FFmpeg).
///
/// Priority:
/// 1. `FOLDSHIFT_FFMPEG_PATH` environment variable.
/// 2. `FOLDSHIFT_MEDIA_PATH` environment variable.
/// 3. Installed and healthy `media` pack in the FoldShift packs directory.
/// 4. Host system `PATH` if `ffmpeg` is installed globally.
pub fn resolve_media_engine() -> Result<PathBuf> {
    if let Ok(path_str) = std::env::var("FOLDSHIFT_FFMPEG_PATH") {
        let p = PathBuf::from(path_str.trim());
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Ok(path_str) = std::env::var("FOLDSHIFT_MEDIA_PATH") {
        let p = PathBuf::from(path_str.trim());
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Some(pack) = foldshift_core::pack::get_installed_engine("media") {
        let entrypoint = pack.resolved_entrypoint();
        if entrypoint.is_file() {
            return Ok(entrypoint);
        }
    }

    // Check system PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let bin_name = if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(FcError::CapabilityUnavailable(
        "media engine (ffmpeg) is not installed. Install the 'media' pack via 'foldshift packs install media' or configure FOLDSHIFT_FFMPEG_PATH".into(),
    ))
}

/// Maximum process output capture buffer (10 MB safety cap).
pub const MAX_OUTPUT_BYTES: u64 = 10 * 1024 * 1024;

/// Executes a media engine subprocess with bounded limits, deadlock prevention,
/// and concurrent pipe draining.
pub fn run_media_engine(engine: &Path, args: &[String]) -> Result<(String, String)> {
    let mut cmd = Command::new(engine);
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        FcError::Conversion(format!(
            "failed to spawn media engine at '{}': {}",
            engine.display(),
            e
        ))
    })?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

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

    // 60-second execution timeout guard
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
                        "media engine timed out after {}s",
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

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();

    if !status.success() {
        let msg = if !stderr_str.trim().is_empty() {
            stderr_str.trim().to_string()
        } else if !stdout_str.trim().is_empty() {
            stdout_str.trim().to_string()
        } else {
            format!("media engine exited with status: {}", status)
        };
        return Err(FcError::Conversion(msg));
    }

    Ok((stdout_str, stderr_str))
}

/// Inspects a media file (audio or video) and returns its metadata, duration, bitrate, and stream properties.
pub fn inspect(path: &Path) -> Result<Inspection> {
    ensure_input(path)?;
    let engine = resolve_media_engine()?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_ascii_lowercase();

    let file_metadata = fs::metadata(path)?;
    let file_bytes = file_metadata.len();

    // Run ffmpeg with -i to probe media streams
    let args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-i".to_string(),
        path.to_string_lossy().to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];

    // Note: FFmpeg exits with error or 0 on probe depending on flags, but probe info is printed to stderr.
    let (_, stderr) = match run_media_engine(&engine, &args) {
        Ok(out) => out,
        Err(FcError::Conversion(msg)) if msg.contains("Stream #") || msg.contains("Duration:") => {
            (String::new(), msg)
        }
        Err(e) => return Err(e),
    };

    let is_video = stderr.contains("Video: ") || is_supported_video_extension(&ext);
    let media_kind = if is_video {
        MediaKind::Video
    } else {
        MediaKind::Audio
    };

    let mut inspection = Inspection::new(path, ext.to_uppercase(), media_kind, file_bytes);

    // Parse duration: "Duration: 00:01:23.45,"
    if let Some(pos) = stderr.find("Duration: ") {
        let slice = &stderr[pos + 10..];
        let dur_str: String = slice
            .chars()
            .take_while(|c| *c != ',' && *c != '\n')
            .collect();
        inspection = inspection.with_property("duration", json!(dur_str.trim()));
    }

    // Parse bitrate: "bitrate: 320 kb/s"
    if let Some(pos) = stderr.find("bitrate: ") {
        let slice = &stderr[pos + 9..];
        let br_str: String = slice
            .chars()
            .take_while(|c| *c != ',' && *c != '\n')
            .collect();
        inspection = inspection.with_property("bitrate", json!(br_str.trim()));
    }

    // Parse Video stream info if present
    if let Some(pos) = stderr.find("Video: ") {
        let slice = &stderr[pos + 7..];
        let line: String = slice
            .chars()
            .take_while(|c| *c != '\n' && *c != '\r')
            .collect();
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if let Some(codec) = parts.first() {
            let codec_clean = codec.split_whitespace().next().unwrap_or(codec);
            inspection = inspection.with_property("video_codec", json!(codec_clean));
        }

        // Search for resolution like "1920x1080"
        for part in &parts {
            let token = part.split_whitespace().next().unwrap_or(part);
            if let Some((w_s, h_s)) = token.split_once('x')
                && let (Ok(w), Ok(h)) = (w_s.parse::<u32>(), h_s.parse::<u32>())
            {
                inspection = inspection.with_property("width", json!(w));
                inspection = inspection.with_property("height", json!(h));
                inspection = inspection.with_property("resolution", json!(format!("{}x{}", w, h)));
                break;
            }
        }

        // Search for framerate like "30 fps"
        for part in &parts {
            if part.ends_with("fps") {
                let num_str = part.trim_end_matches("fps").trim();
                if let Ok(fps) = num_str.parse::<f32>() {
                    inspection = inspection.with_property("fps", json!(fps));
                    break;
                }
            }
        }
    }

    // Parse Audio stream info
    if let Some(pos) = stderr.find("Audio: ") {
        let slice = &stderr[pos + 7..];
        let line: String = slice
            .chars()
            .take_while(|c| *c != '\n' && *c != '\r')
            .collect();
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if let Some(codec) = parts.first() {
            let codec_clean = codec.split_whitespace().next().unwrap_or(codec);
            let prop_key = if is_video { "audio_codec" } else { "codec" };
            inspection = inspection.with_property(prop_key, json!(codec_clean));
        }
        if let Some(sr) = parts.get(1) {
            inspection = inspection.with_property("sample_rate", json!(*sr));
        }
        if let Some(channels) = parts.get(2) {
            inspection = inspection.with_property("channels", json!(*channels));
        }
    }

    Ok(inspection)
}

/// Converts an audio file to the target format with safe publication and atomic isolation.
pub fn convert(input: &Path, output: &Path, options: AudioConvertOptions) -> Result<()> {
    ensure_input(input)?;

    let target_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| FcError::UnsupportedFormat(output.display().to_string()))?;

    if !is_supported_audio_extension(target_ext) {
        return Err(FcError::UnsupportedFormat(target_ext.to_string()));
    }

    if options.channels == Some(0) {
        return Err(FcError::InvalidOption(
            "channels count must be greater than zero".into(),
        ));
    }

    if options.sample_rate == Some(0) {
        return Err(FcError::InvalidOption(
            "sample rate must be greater than zero".into(),
        ));
    }

    let is_same_file = is_same_file(input, output)?;
    if output.exists() && !options.force && !is_same_file {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    if is_same_file && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }

    let engine = resolve_media_engine()?;

    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if !parent.is_dir() {
        fs::create_dir_all(parent)?;
    }

    // Atomic staging file
    let temporary = tempfile::Builder::new()
        .suffix(&format!(".{}", target_ext))
        .tempfile_in(parent)?;
    let temp_path = temporary.path().to_path_buf();

    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
    ];

    if let Some(sr) = options.sample_rate {
        args.push("-ar".to_string());
        args.push(sr.to_string());
    }

    if let Some(ch) = options.channels {
        args.push("-ac".to_string());
        args.push(ch.to_string());
    }

    if let Some(br) = &options.bitrate {
        args.push("-b:a".to_string());
        args.push(br.clone());
    }

    args.push(temp_path.to_string_lossy().to_string());

    let run_res = run_media_engine(&engine, &args);
    if let Err(e) = run_res {
        let _ = temporary.close();
        return Err(e);
    }

    // Atomically persist to output
    if options.force {
        temporary
            .persist(output)
            .map_err(|e| FcError::Io(e.error))?;
    } else {
        temporary.persist_noclobber(output).map_err(|e| {
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                FcError::OutputExists(output.to_path_buf())
            } else {
                FcError::Io(e.error)
            }
        })?;
    }

    Ok(())
}

/// Converts and transcodes a video file to the target format with atomic publication and option validation.
pub fn convert_video(input: &Path, output: &Path, options: VideoConvertOptions) -> Result<()> {
    ensure_input(input)?;

    let target_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| FcError::UnsupportedFormat(output.display().to_string()))?;

    if !is_supported_video_extension(target_ext) {
        return Err(FcError::UnsupportedFormat(target_ext.to_string()));
    }

    let (target_w, target_h) = if let Some((w, h)) = options.resolution {
        (Some(w), Some(h))
    } else {
        (options.width, options.height)
    };

    if target_w == Some(0) || target_h == Some(0) {
        return Err(FcError::InvalidOption(
            "resolution dimensions must be greater than zero".into(),
        ));
    }

    if let Some(fps) = options.fps
        && fps <= 0.0
    {
        return Err(FcError::InvalidOption(
            "framerate must be greater than zero".into(),
        ));
    }

    let is_same = is_same_file(input, output)?;
    if output.exists() && !options.force && !is_same {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    if is_same && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }

    let engine = resolve_media_engine()?;

    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if !parent.is_dir() {
        fs::create_dir_all(parent)?;
    }

    let temporary = tempfile::Builder::new()
        .suffix(&format!(".{}", target_ext))
        .tempfile_in(parent)?;
    let temp_path = temporary.path().to_path_buf();

    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
    ];

    match (target_w, target_h) {
        (Some(w), Some(h)) => {
            args.push("-vf".to_string());
            args.push(format!("scale={}:{}", w, h));
        }
        (Some(w), None) => {
            args.push("-vf".to_string());
            args.push(format!("scale={}:-2", w));
        }
        (None, Some(h)) => {
            args.push("-vf".to_string());
            args.push(format!("scale=-2:{}", h));
        }
        (None, None) => {}
    }

    if let Some(fps) = options.fps {
        args.push("-r".to_string());
        args.push(format!("{}", fps));
    }

    if let Some(crf) = options.crf {
        args.push("-crf".to_string());
        args.push(crf.to_string());
    }

    if let Some(codec) = &options.video_codec {
        args.push("-c:v".to_string());
        args.push(codec.clone());
    }

    if let Some(vbr) = &options.video_bitrate {
        args.push("-b:v".to_string());
        args.push(vbr.clone());
    }

    if options.mute_audio {
        args.push("-an".to_string());
    } else if let Some(abr) = &options.audio_bitrate {
        args.push("-b:a".to_string());
        args.push(abr.clone());
    }

    let lower_ext = target_ext.to_ascii_lowercase();
    if lower_ext == "mp4" || lower_ext == "m4v" {
        args.push("-movflags".to_string());
        args.push("+faststart".to_string());
    }

    args.push(temp_path.to_string_lossy().to_string());

    let run_res = run_media_engine(&engine, &args);
    if let Err(e) = run_res {
        let _ = temporary.close();
        return Err(e);
    }

    if options.force {
        temporary
            .persist(output)
            .map_err(|e| FcError::Io(e.error))?;
    } else {
        temporary.persist_noclobber(output).map_err(|e| {
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                FcError::OutputExists(output.to_path_buf())
            } else {
                FcError::Io(e.error)
            }
        })?;
    }

    Ok(())
}

/// Extracts the audio stream from a video file into an audio destination file.
pub fn extract_audio(input: &Path, output: &Path, force: bool) -> Result<()> {
    ensure_input(input)?;

    let target_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| FcError::UnsupportedFormat(output.display().to_string()))?;

    if !is_supported_audio_extension(target_ext) {
        return Err(FcError::UnsupportedFormat(format!(
            "unsupported audio destination extension '{}' for extraction",
            target_ext
        )));
    }

    let is_same = is_same_file(input, output)?;
    if output.exists() && !force && !is_same {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    if is_same && !force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }

    let engine = resolve_media_engine()?;

    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if !parent.is_dir() {
        fs::create_dir_all(parent)?;
    }

    let temporary = tempfile::Builder::new()
        .suffix(&format!(".{}", target_ext))
        .tempfile_in(parent)?;
    let temp_path = temporary.path().to_path_buf();

    let args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
        "-vn".to_string(),
        temp_path.to_string_lossy().to_string(),
    ];

    let run_res = run_media_engine(&engine, &args);
    if let Err(e) = run_res {
        let _ = temporary.close();
        return Err(e);
    }

    if force {
        temporary
            .persist(output)
            .map_err(|e| FcError::Io(e.error))?;
    } else {
        temporary.persist_noclobber(output).map_err(|e| {
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                FcError::OutputExists(output.to_path_buf())
            } else {
                FcError::Io(e.error)
            }
        })?;
    }

    Ok(())
}

/// Batch converts multiple audio files with bounded concurrency, progress reporting, and cancellation.
#[allow(clippy::too_many_arguments)]
pub fn batch_convert_audio(
    inputs: &[PathBuf],
    target_ext: &str,
    output_dir: &Path,
    pattern: Option<&str>,
    options: AudioConvertOptions,
    concurrency: usize,
    cancel_token: &foldshift_core::batch::CancellationToken,
    progress: impl FnMut(foldshift_core::batch::BatchProgress<'_>),
) -> Result<foldshift_core::batch::BatchSummary> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    let items =
        foldshift_core::batch::prepare_batch_items(inputs, output_dir, pattern, target_ext)?;

    let max_parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let effective_concurrency = if concurrency == 0 {
        1
    } else {
        concurrency.min(max_parallelism)
    };

    let summary = foldshift_core::batch::execute_bounded_batch(
        &items,
        effective_concurrency,
        cancel_token,
        progress,
        |item| {
            cancel_token.check_cancelled()?;
            convert(&item.input, &item.output, options.clone())?;
            let written = fs::metadata(&item.output).map(|m| m.len()).unwrap_or(0);
            Ok(written)
        },
    );

    Ok(summary)
}

/// Batch converts multiple audio files sequentially (for backwards compatibility).
pub fn batch_convert(
    inputs: &[PathBuf],
    target_ext: &str,
    output_dir: &Path,
    pattern: Option<&str>,
    options: AudioConvertOptions,
    cancel_token: &foldshift_core::batch::CancellationToken,
    progress: impl FnMut(foldshift_core::batch::BatchProgress<'_>),
) -> Result<foldshift_core::batch::BatchSummary> {
    batch_convert_audio(
        inputs,
        target_ext,
        output_dir,
        pattern,
        options,
        1,
        cancel_token,
        progress,
    )
}

/// Batch converts multiple video files with measured bounded concurrency, progress reporting, and cancellation.
#[allow(clippy::too_many_arguments)]
pub fn batch_convert_video(
    inputs: &[PathBuf],
    target_ext: &str,
    output_dir: &Path,
    pattern: Option<&str>,
    options: VideoConvertOptions,
    concurrency: usize,
    cancel_token: &foldshift_core::batch::CancellationToken,
    progress: impl FnMut(foldshift_core::batch::BatchProgress<'_>),
) -> Result<foldshift_core::batch::BatchSummary> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    let items =
        foldshift_core::batch::prepare_batch_items(inputs, output_dir, pattern, target_ext)?;

    let max_parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let effective_concurrency = if concurrency == 0 {
        1
    } else {
        concurrency.min(max_parallelism)
    };

    let summary = foldshift_core::batch::execute_bounded_batch(
        &items,
        effective_concurrency,
        cancel_token,
        progress,
        |item| {
            cancel_token.check_cancelled()?;
            convert_video(&item.input, &item.output, options.clone())?;
            let written = fs::metadata(&item.output).map(|m| m.len()).unwrap_or(0);
            Ok(written)
        },
    );

    Ok(summary)
}

fn ensure_input(path: &Path) -> Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(FcError::InputMissing(path.to_path_buf()));
        }
        Err(err) => return Err(FcError::Io(err)),
    };
    if metadata.is_dir() {
        return Err(FcError::InvalidOption(format!(
            "input path is a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn is_same_file(path_a: &Path, path_b: &Path) -> Result<bool> {
    if path_a == path_b {
        return Ok(true);
    }
    if path_a.exists() && path_b.exists() {
        let meta_a = fs::metadata(path_a)?;
        let meta_b = fs::metadata(path_b)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino() {
                return Ok(true);
            }
        }
        #[cfg(windows)]
        {
            if let (Ok(canon_a), Ok(canon_b)) = (fs::canonicalize(path_a), fs::canonicalize(path_b))
                && canon_a == canon_b
            {
                return Ok(true);
            }
        }
        let _ = (meta_a, meta_b);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_audio_extensions() {
        assert!(is_supported_audio_extension("mp3"));
        assert!(is_supported_audio_extension("wav"));
        assert!(is_supported_audio_extension(".FLAC"));
        assert!(!is_supported_audio_extension("png"));
        assert!(!is_supported_audio_extension("pdf"));
        assert!(!is_supported_audio_extension("mp4"));
    }

    #[test]
    fn test_supported_video_extensions() {
        assert!(is_supported_video_extension("mp4"));
        assert!(is_supported_video_extension("mkv"));
        assert!(is_supported_video_extension("webm"));
        assert!(is_supported_video_extension(".MOV"));
        assert!(is_supported_video_extension("avi"));
        assert!(!is_supported_video_extension("mp3"));
        assert!(!is_supported_video_extension("png"));
    }

    #[test]
    fn uninstalled_engine_returns_capability_unavailable() {
        let old_ffmpeg = std::env::var("FOLDSHIFT_FFMPEG_PATH").ok();
        let old_media = std::env::var("FOLDSHIFT_MEDIA_PATH").ok();
        let old_path = std::env::var("PATH").ok();

        unsafe {
            std::env::set_var("FOLDSHIFT_FFMPEG_PATH", "nonexistent_ffmpeg_bin");
            std::env::set_var("FOLDSHIFT_MEDIA_PATH", "nonexistent_media_bin");
            std::env::set_var("PATH", "");
        }

        let res = resolve_media_engine();
        assert!(matches!(res, Err(FcError::CapabilityUnavailable(_))));

        unsafe {
            if let Some(v) = old_ffmpeg {
                std::env::set_var("FOLDSHIFT_FFMPEG_PATH", v);
            } else {
                std::env::remove_var("FOLDSHIFT_FFMPEG_PATH");
            }
            if let Some(v) = old_media {
                std::env::set_var("FOLDSHIFT_MEDIA_PATH", v);
            } else {
                std::env::remove_var("FOLDSHIFT_MEDIA_PATH");
            }
            if let Some(v) = old_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
        }
    }
}
