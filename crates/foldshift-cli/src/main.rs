use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
#[cfg(feature = "image")]
use foldshift_image::EncodeOptions;

#[derive(Debug, Parser)]
#[command(
    name = "foldshift",
    version,
    about = "Private, local-first file conversion"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show installed and planned operations.
    Capabilities {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect or manage optional capability engine packs.
    Packs {
        #[command(subcommand)]
        command: PacksCommand,
    },
    /// PDF-specific operations.
    Pdf {
        #[command(subcommand)]
        command: PdfCommand,
    },
    /// Audio-specific operations.
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    /// Video-specific operations.
    Video {
        #[command(subcommand)]
        command: VideoCommand,
    },
    /// Inspect a supported local file.
    Inspect {
        file: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Convert an image based on the output extension.
    #[cfg(feature = "image")]
    Convert(ConvertArgs),
    /// Image-specific operations.
    #[cfg(feature = "image")]
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
    /// Batch operations for processing multiple files sequentially.
    Batch {
        #[command(subcommand)]
        command: BatchCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PacksCommand {
    /// List installed and available capability packs.
    List {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Install an engine capability pack from an offline archive.
    Install {
        /// Capability to install (e.g. 'pdf', 'media').
        #[arg(value_name = "CAPABILITY")]
        capability: Option<String>,
        /// Path to offline pack archive (.zip).
        #[arg(long)]
        from_file: Option<PathBuf>,
        /// Expected SHA-256 checksum to verify against the archive.
        #[arg(long)]
        sha256: Option<String>,
    },
    /// Update an installed engine capability pack to a new version.
    Update {
        /// Capability to update (e.g. 'pdf', 'media').
        #[arg(value_name = "CAPABILITY")]
        capability: Option<String>,
        /// Path to offline pack archive (.zip).
        #[arg(long)]
        from_file: Option<PathBuf>,
        /// Expected SHA-256 checksum to verify against the archive.
        #[arg(long)]
        sha256: Option<String>,
    },
    /// Remove an installed engine capability pack.
    Remove {
        /// Capability to remove (e.g. 'pdf', 'media').
        capability: String,
    },
}

#[derive(Debug, Subcommand)]
enum PdfCommand {
    /// Inspect a PDF file's page count and structure.
    Inspect {
        file: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Merge multiple PDF documents into a single output file.
    Merge {
        /// Input PDF files to merge in order.
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        /// Destination output file.
        #[arg(short, long)]
        output: PathBuf,
        /// Replace an existing output file.
        #[arg(long)]
        force: bool,
    },
    /// Extract page ranges from a PDF document.
    Extract {
        /// Source PDF file.
        input: PathBuf,
        /// Destination output file.
        output: PathBuf,
        /// Page range to extract (e.g. '1-3', '1,5,7', '2').
        #[arg(long)]
        pages: String,
        /// Replace an existing output file.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AudioCommand {
    /// Inspect an audio file's duration, bitrate, sample rate, channels, and codec.
    Inspect {
        file: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Convert an audio file to a target format.
    Convert(AudioConvertArgs),
}

#[derive(Debug, Args)]
struct AudioConvertArgs {
    input: PathBuf,
    output: PathBuf,
    /// Target audio bitrate (e.g. "128k", "192k", "320k").
    #[arg(long)]
    bitrate: Option<String>,
    /// Target audio sample rate in Hz (e.g. 44100, 48000).
    #[arg(long)]
    sample_rate: Option<u32>,
    /// Target channels count (1 for mono, 2 for stereo).
    #[arg(long)]
    channels: Option<u8>,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Subcommand)]
enum VideoCommand {
    /// Inspect a video file's duration, bitrate, resolution, fps, and codecs.
    Inspect {
        file: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Convert/transcode a video file to a target format or adjust resolution/fps.
    Convert(VideoConvertArgs),
    /// Extract the audio stream from a video file into an audio format.
    ExtractAudio(VideoExtractAudioArgs),
}

#[derive(Debug, Args)]
struct VideoConvertArgs {
    input: PathBuf,
    output: PathBuf,
    /// Target resolution in WIDTHxHEIGHT format (e.g. "1920x1080").
    #[arg(long)]
    resolution: Option<String>,
    /// Maximum output width in pixels. Preserves aspect ratio if height is omitted.
    #[arg(long)]
    width: Option<u32>,
    /// Maximum output height in pixels. Preserves aspect ratio if width is omitted.
    #[arg(long)]
    height: Option<u32>,
    /// Target frame rate in FPS (e.g. 24, 30, 60).
    #[arg(long)]
    fps: Option<f32>,
    /// Constant Rate Factor (CRF) quality (e.g. 18-28). Lower = higher quality.
    #[arg(long)]
    crf: Option<u8>,
    /// Target video bitrate (e.g. "2M", "1500k").
    #[arg(long)]
    video_bitrate: Option<String>,
    /// Target audio bitrate (e.g. "128k", "192k").
    #[arg(long)]
    audio_bitrate: Option<String>,
    /// Strip audio stream from output video.
    #[arg(long)]
    mute_audio: bool,
    /// Target video codec (e.g. "libx264", "libvpx-vp9").
    #[arg(long)]
    video_codec: Option<String>,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct VideoExtractAudioArgs {
    input: PathBuf,
    output: PathBuf,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[cfg(feature = "image")]
#[derive(Debug, Subcommand)]
enum ImageCommand {
    Convert(ConvertArgs),
    Resize(ResizeArgs),
}

#[cfg(feature = "image")]
#[derive(Debug, Args)]
struct ConvertArgs {
    input: PathBuf,
    output: PathBuf,
    /// JPEG quality from 1 to 100. Other encoders currently ignore it.
    #[arg(long, default_value_t = 82, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[cfg(feature = "image")]
#[derive(Debug, Args)]
struct ResizeArgs {
    input: PathBuf,
    output: PathBuf,
    /// Maximum output width.
    #[arg(long)]
    width: Option<u32>,
    /// Maximum output height.
    #[arg(long)]
    height: Option<u32>,
    /// JPEG quality from 1 to 100. Other encoders currently ignore it.
    #[arg(long, default_value_t = 82, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Subcommand)]
enum BatchCommand {
    /// Convert multiple images sequentially with progress reporting.
    #[cfg(feature = "image")]
    Convert(BatchConvertArgs),
    /// Resize multiple images sequentially with progress reporting.
    #[cfg(feature = "image")]
    Resize(BatchResizeArgs),
    /// Convert multiple audio files sequentially with progress reporting.
    Audio(BatchAudioArgs),
    /// Convert multiple video files with measured bounded concurrency.
    Video(BatchVideoArgs),
}

#[derive(Debug, Args)]
struct BatchAudioArgs {
    /// Input audio files to convert.
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
    /// Destination audio format / extension (e.g. 'mp3', 'wav', 'flac', 'ogg', 'm4a', 'aac', 'opus').
    #[arg(long, short = 't')]
    to: String,
    /// Directory where converted files will be saved.
    #[arg(long, short = 'o')]
    out_dir: PathBuf,
    /// Naming pattern for output files (e.g. '{stem}.{ext}', '{stem}-converted.{ext}', '{index:03}_{stem}.{ext}').
    #[arg(long)]
    pattern: Option<String>,
    /// Target audio bitrate (e.g. "128k", "192k", "320k").
    #[arg(long)]
    bitrate: Option<String>,
    /// Target audio sample rate in Hz (e.g. 44100, 48000).
    #[arg(long)]
    sample_rate: Option<u32>,
    /// Target channels count (1 for mono, 2 for stereo).
    #[arg(long)]
    channels: Option<u8>,
    /// Concurrency limit for audio workers (defaults to 1).
    #[arg(short = 'j', long, default_value_t = 1)]
    concurrency: usize,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON summary upon completion.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct BatchVideoArgs {
    /// Input video files to convert.
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
    /// Destination video format / extension (e.g. 'mp4', 'webm', 'mkv', 'avi', 'mov', 'flv', 'wmv', 'm4v').
    #[arg(long, short = 't')]
    to: String,
    /// Directory where converted files will be saved.
    #[arg(long, short = 'o')]
    out_dir: PathBuf,
    /// Naming pattern for output files (e.g. '{stem}.{ext}', '{stem}-converted.{ext}', '{index:03}_{stem}.{ext}').
    #[arg(long)]
    pattern: Option<String>,
    /// Target resolution in WIDTHxHEIGHT format (e.g. "1920x1080").
    #[arg(long)]
    resolution: Option<String>,
    /// Maximum output width in pixels. Preserves aspect ratio if height is omitted.
    #[arg(long)]
    width: Option<u32>,
    /// Maximum output height in pixels. Preserves aspect ratio if width is omitted.
    #[arg(long)]
    height: Option<u32>,
    /// Target frame rate in FPS (e.g. 24, 30, 60).
    #[arg(long)]
    fps: Option<f32>,
    /// Constant Rate Factor (CRF) quality (e.g. 18-28). Lower = higher quality.
    #[arg(long)]
    crf: Option<u8>,
    /// Target video bitrate (e.g. "2M", "1500k").
    #[arg(long)]
    video_bitrate: Option<String>,
    /// Target audio bitrate (e.g. "128k", "192k").
    #[arg(long)]
    audio_bitrate: Option<String>,
    /// Strip audio stream from output video.
    #[arg(long)]
    mute_audio: bool,
    /// Target video codec (e.g. "libx264", "libvpx-vp9").
    #[arg(long)]
    video_codec: Option<String>,
    /// Concurrency limit for video workers (defaults to 1 to prevent system starvation).
    #[arg(short = 'j', long, default_value_t = 1)]
    concurrency: usize,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON summary upon completion.
    #[arg(long)]
    json: bool,
}

#[cfg(feature = "image")]
#[derive(Debug, Args)]
struct BatchConvertArgs {
    /// Input files to convert.
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
    /// Destination format / extension (e.g. 'png', 'jpg', 'webp').
    #[arg(long, short = 't')]
    to: String,
    /// Directory where converted files will be saved.
    #[arg(long, short = 'o')]
    out_dir: PathBuf,
    /// Naming pattern for output files (e.g. '{stem}.{ext}', '{stem}-converted.{ext}', '{index:03}_{stem}.{ext}').
    #[arg(long)]
    pattern: Option<String>,
    /// JPEG quality from 1 to 100.
    #[arg(long, default_value_t = 82, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON summary upon completion.
    #[arg(long)]
    json: bool,
}

#[cfg(feature = "image")]
#[derive(Debug, Args)]
struct BatchResizeArgs {
    /// Input files to resize.
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
    /// Directory where resized files will be saved.
    #[arg(long, short = 'o')]
    out_dir: PathBuf,
    /// Maximum output width.
    #[arg(long)]
    width: Option<u32>,
    /// Maximum output height.
    #[arg(long)]
    height: Option<u32>,
    /// Optional target format (defaults to source format if not specified).
    #[arg(long, short = 't')]
    to: Option<String>,
    /// Naming pattern for output files (e.g. '{stem}.{ext}', 'thumb_{stem}.{ext}').
    #[arg(long)]
    pattern: Option<String>,
    /// JPEG quality from 1 to 100.
    #[arg(long, default_value_t = 82, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON summary upon completion.
    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capabilities { json } => capabilities(json)?,
        Command::Packs { command } => match command {
            PacksCommand::List { json } => packs_list(json)?,
            PacksCommand::Install {
                capability,
                from_file,
                sha256,
            } => packs_install(capability, from_file, sha256)?,
            PacksCommand::Update {
                capability,
                from_file,
                sha256,
            } => packs_update(capability, from_file, sha256)?,
            PacksCommand::Remove { capability } => packs_remove(capability)?,
        },
        Command::Pdf { command } => match command {
            PdfCommand::Inspect { file, json } => pdf_inspect(file, json)?,
            PdfCommand::Merge {
                inputs,
                output,
                force,
            } => pdf_merge(inputs, output, force)?,
            PdfCommand::Extract {
                input,
                output,
                pages,
                force,
            } => pdf_extract(input, output, pages, force)?,
        },
        Command::Audio { command } => match command {
            AudioCommand::Inspect { file, json } => audio_inspect(file, json)?,
            AudioCommand::Convert(args) => audio_convert(args)?,
        },
        Command::Video { command } => match command {
            VideoCommand::Inspect { file, json } => video_inspect(file, json)?,
            VideoCommand::Convert(args) => video_convert(args)?,
            VideoCommand::ExtractAudio(args) => video_extract_audio(args)?,
        },
        Command::Inspect { file, json } => inspect(file, json)?,
        #[cfg(feature = "image")]
        Command::Convert(args)
        | Command::Image {
            command: ImageCommand::Convert(args),
        } => convert(args)?,
        #[cfg(feature = "image")]
        Command::Image {
            command: ImageCommand::Resize(args),
        } => resize(args)?,
        Command::Batch { command } => match command {
            #[cfg(feature = "image")]
            BatchCommand::Convert(args) => batch_convert_cmd(args)?,
            #[cfg(feature = "image")]
            BatchCommand::Resize(args) => batch_resize_cmd(args)?,
            BatchCommand::Audio(args) => batch_audio_cmd(args)?,
            BatchCommand::Video(args) => batch_video_cmd(args)?,
        },
    }
    Ok(())
}

fn capabilities(json: bool) -> Result<()> {
    let is_image_available = cfg!(feature = "image");
    let is_pdf_available = foldshift_core::pack::is_capability_installed("pdf");
    let is_media_available = foldshift_core::pack::is_capability_installed("media")
        || foldshift_media::resolve_media_engine().is_ok();
    let operations = foldshift_core::operations_with(|cap| match cap {
        "image" => is_image_available,
        "pdf" => is_pdf_available,
        "media" => is_media_available,
        _ => false,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&operations)?);
        return Ok(());
    }

    println!("CORE   installed");
    #[cfg(feature = "image")]
    println!(
        "IMAGE  installed ({})",
        foldshift_image::SUPPORTED_EXTENSIONS.join(", ")
    );
    #[cfg(not(feature = "image"))]
    println!("IMAGE  not installed (optional feature disabled)");

    if let Some(pdf_pack) = foldshift_core::pack::get_installed_engine("pdf") {
        println!("PDF    installed ({} v{})", pdf_pack.name, pdf_pack.version);
    } else {
        println!("PDF    not installed (run 'foldshift packs list')");
    }

    if let Some(media_pack) = foldshift_core::pack::get_installed_engine("media") {
        println!(
            "MEDIA  installed ({} v{})",
            media_pack.name, media_pack.version
        );
    } else if let Ok(engine) = foldshift_media::resolve_media_engine() {
        let display = engine
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ffmpeg");
        println!("MEDIA  installed (system/env: {})", display);
    } else {
        println!("MEDIA  not installed (run 'foldshift packs list')");
    }

    println!();
    for operation in operations {
        let marker = if operation.available { "yes" } else { "no " };
        println!("{marker}  {:<16} {}", operation.id, operation.summary);
    }
    Ok(())
}

fn packs_list(json: bool) -> Result<()> {
    let statuses = foldshift_core::pack::list_pack_statuses();
    if json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
        return Ok(());
    }

    println!("Available capability packs:\n");
    for status in &statuses {
        let state_str = if status.installed {
            if status.healthy {
                format!(
                    "installed (v{}) [healthy]",
                    status.version.as_deref().unwrap_or("unknown")
                )
            } else {
                "corrupted / entrypoint missing [failed]".to_string()
            }
        } else {
            "not installed (planned)".to_string()
        };

        println!(
            "{:<6} {} ({})",
            status.capability.to_uppercase(),
            status.id,
            status.name
        );
        println!("       Status: {}", state_str);
        println!("       Summary: {}", status.summary);
        if let Some(entrypoint) = &status.entrypoint {
            println!("       Entrypoint: {}", entrypoint.display());
        }
        if let Some(install_dir) = &status.install_dir {
            println!("       Location: {}", install_dir.display());
        }
        println!();
    }

    println!(
        "Packs directory: {}",
        foldshift_core::pack::packs_dir().display()
    );
    println!("Run 'foldshift capabilities' to inspect active operations.");
    Ok(())
}

fn packs_install(
    capability: Option<String>,
    from_file: Option<PathBuf>,
    sha256: Option<String>,
) -> Result<()> {
    let Some(archive_path) = from_file else {
        let cap_hint = capability.as_deref().unwrap_or("<capability>");
        anyhow::bail!(
            "Offline mode: automatic network download is not enabled.\n\
             To install an optional engine pack offline, download the verified pack archive (.zip) and run:\n  \
             foldshift packs install {} --from-file <path-to-archive.zip>",
            cap_hint
        );
    };

    if !archive_path.is_file() {
        anyhow::bail!("archive file does not exist: {}", archive_path.display());
    }

    let packs_dir = foldshift_core::pack::packs_dir();
    let options = foldshift_core::pack::InstallOptions {
        expected_capability: capability.as_deref(),
        expected_sha256: sha256.as_deref(),
    };

    let installed =
        foldshift_core::pack::install_pack_from_archive(&archive_path, &packs_dir, &options)?;
    println!(
        "Successfully installed pack '{}' v{} for capability '{}'.",
        installed.name, installed.version, installed.capability
    );
    println!("Location: {}", installed.install_dir.display());
    println!("Entrypoint: {}", installed.resolved_entrypoint().display());
    Ok(())
}

fn packs_update(
    capability: Option<String>,
    from_file: Option<PathBuf>,
    sha256: Option<String>,
) -> Result<()> {
    let Some(archive_path) = from_file else {
        let cap_hint = capability.as_deref().unwrap_or("<capability>");
        anyhow::bail!(
            "Offline mode: automatic network download is not enabled.\n\
             To update an engine pack offline, download the verified pack archive (.zip) and run:\n  \
             foldshift packs update {} --from-file <path-to-archive.zip>",
            cap_hint
        );
    };

    if !archive_path.is_file() {
        anyhow::bail!("archive file does not exist: {}", archive_path.display());
    }

    let packs_dir = foldshift_core::pack::packs_dir();
    let options = foldshift_core::pack::InstallOptions {
        expected_capability: capability.as_deref(),
        expected_sha256: sha256.as_deref(),
    };

    let updated =
        foldshift_core::pack::update_pack_from_archive(&archive_path, &packs_dir, &options)?;
    println!(
        "Successfully updated pack '{}' to v{} for capability '{}'.",
        updated.name, updated.version, updated.capability
    );
    println!("Location: {}", updated.install_dir.display());
    Ok(())
}

fn packs_remove(capability: String) -> Result<()> {
    let packs_dir = foldshift_core::pack::packs_dir();
    let removed = foldshift_core::pack::remove_pack(&capability, &packs_dir)?;
    println!(
        "Successfully removed pack '{}' v{} for capability '{}'.",
        removed.name, removed.version, removed.capability
    );
    println!("Cleaned up directory: {}", removed.install_dir.display());
    Ok(())
}

fn inspect(file: PathBuf, json_output: bool) -> Result<()> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "pdf" {
        return pdf_inspect(file, json_output);
    }

    if foldshift_media::is_supported_audio_extension(&ext) {
        return audio_inspect(file, json_output);
    }

    if foldshift_media::is_supported_video_extension(&ext) {
        return video_inspect(file, json_output);
    }

    #[cfg(feature = "image")]
    {
        let inspection = foldshift_image::inspect(&file)?;
        if json_output {
            println!("{}", serde_json::to_string_pretty(&inspection)?);
        } else {
            println!("File    {}", inspection.path.display());
            println!("Type    {}", inspection.format);
            println!("Size    {} bytes", inspection.bytes);
            for (key, value) in inspection.properties {
                println!("{:<8}{}", title_case(&key), plain_json(value));
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "image"))]
    anyhow::bail!("unsupported file format for inspection: {}", file.display());
}

#[cfg(feature = "image")]
fn convert(args: ConvertArgs) -> Result<()> {
    foldshift_image::convert(
        &args.input,
        &args.output,
        EncodeOptions {
            quality: args.quality,
            force: args.force,
        },
    )?;
    println!(
        "Converted {} -> {}",
        args.input.display(),
        args.output.display()
    );
    Ok(())
}

#[cfg(feature = "image")]
fn resize(args: ResizeArgs) -> Result<()> {
    let (width, height) = foldshift_image::resize(
        &args.input,
        &args.output,
        args.width,
        args.height,
        EncodeOptions {
            quality: args.quality,
            force: args.force,
        },
    )?;
    println!(
        "Resized {} -> {} ({}x{})",
        args.input.display(),
        args.output.display(),
        width,
        height
    );
    Ok(())
}

#[cfg(feature = "image")]
fn batch_convert_cmd(args: BatchConvertArgs) -> Result<()> {
    let cancel_token = foldshift_core::batch::CancellationToken::new();
    let handler_token = cancel_token.clone();
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let interrupt_clone = interrupt_count.clone();

    let _ = ctrlc::set_handler(move || {
        let prev = interrupt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 0 {
            eprintln!(
                "\nCancellation requested (Ctrl+C). Safely stopping batch after current item..."
            );
            handler_token.cancel();
        } else {
            eprintln!("\nSecond interrupt received. Terminating immediately.");
            std::process::exit(130);
        }
    });

    let json_mode = args.json;
    let summary = foldshift_image::batch_convert(
        &args.inputs,
        &args.to,
        &args.out_dir,
        args.pattern.as_deref(),
        EncodeOptions {
            quality: args.quality,
            force: args.force,
        },
        &cancel_token,
        |ev| {
            if !json_mode {
                render_batch_progress(&ev);
            }
        },
    )?;

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        render_batch_summary(&summary);
    }

    if summary.has_failures_or_cancellations() {
        anyhow::bail!(
            "Batch completed with issues: {} succeeded, {} failed, {} cancelled",
            summary.succeeded,
            summary.failed,
            summary.cancelled
        );
    }

    Ok(())
}

#[cfg(feature = "image")]
fn batch_resize_cmd(args: BatchResizeArgs) -> Result<()> {
    let cancel_token = foldshift_core::batch::CancellationToken::new();
    let handler_token = cancel_token.clone();
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let interrupt_clone = interrupt_count.clone();

    let _ = ctrlc::set_handler(move || {
        let prev = interrupt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 0 {
            eprintln!(
                "\nCancellation requested (Ctrl+C). Safely stopping batch after current item..."
            );
            handler_token.cancel();
        } else {
            eprintln!("\nSecond interrupt received. Terminating immediately.");
            std::process::exit(130);
        }
    });

    let json_mode = args.json;
    let summary = foldshift_image::batch_resize(
        &args.inputs,
        args.to.as_deref(),
        &args.out_dir,
        args.pattern.as_deref(),
        foldshift_image::BatchResizeOptions {
            width: args.width,
            height: args.height,
            quality: args.quality,
            force: args.force,
        },
        &cancel_token,
        |ev| {
            if !json_mode {
                render_batch_progress(&ev);
            }
        },
    )?;

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        render_batch_summary(&summary);
    }

    if summary.has_failures_or_cancellations() {
        anyhow::bail!(
            "Batch completed with issues: {} succeeded, {} failed, {} cancelled",
            summary.succeeded,
            summary.failed,
            summary.cancelled
        );
    }

    Ok(())
}

fn batch_audio_cmd(args: BatchAudioArgs) -> Result<()> {
    let cancel_token = foldshift_core::batch::CancellationToken::new();
    let handler_token = cancel_token.clone();
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let interrupt_clone = interrupt_count.clone();

    let _ = ctrlc::set_handler(move || {
        let prev = interrupt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 0 {
            eprintln!(
                "\nCancellation requested (Ctrl+C). Safely stopping batch after current item..."
            );
            handler_token.cancel();
        } else {
            eprintln!("\nSecond interrupt received. Terminating immediately.");
            std::process::exit(130);
        }
    });

    let json_mode = args.json;
    let summary = foldshift_media::batch_convert_audio(
        &args.inputs,
        &args.to,
        &args.out_dir,
        args.pattern.as_deref(),
        foldshift_media::AudioConvertOptions {
            bitrate: args.bitrate,
            sample_rate: args.sample_rate,
            channels: args.channels,
            force: args.force,
        },
        args.concurrency,
        &cancel_token,
        |ev| {
            if !json_mode {
                render_batch_progress(&ev);
            }
        },
    )?;

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        render_batch_summary(&summary);
    }

    if summary.has_failures_or_cancellations() {
        anyhow::bail!(
            "Batch completed with issues: {} succeeded, {} failed, {} cancelled",
            summary.succeeded,
            summary.failed,
            summary.cancelled
        );
    }

    Ok(())
}

fn render_batch_progress(ev: &foldshift_core::batch::BatchProgress<'_>) {
    use foldshift_core::batch::{BatchProgress, ItemStatus};
    match ev {
        BatchProgress::Started { total } => {
            println!("Batch started: {total} item(s) queued for processing");
        }
        BatchProgress::ItemStarted {
            index,
            total,
            input,
            ..
        } => {
            let filename = input.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            print!("[{index}/{total}] Processing {filename} ... ");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        BatchProgress::ItemCompleted { result, .. } => match result.status {
            ItemStatus::Success => {
                let bytes = result.bytes_written.unwrap_or(0);
                println!("OK ({} bytes, {}ms)", bytes, result.duration_ms);
            }
            ItemStatus::Failed => {
                println!(
                    "FAILED: {}",
                    result.error.as_deref().unwrap_or("unknown error")
                );
            }
            ItemStatus::Cancelled => {
                println!("CANCELLED");
            }
            ItemStatus::Skipped => {
                println!("SKIPPED");
            }
        },
        BatchProgress::Cancelled {
            completed,
            remaining,
        } => {
            println!("\nBatch cancelled: {completed} completed, {remaining} skipped");
        }
        BatchProgress::Finished { .. } => {}
    }
}

fn render_batch_summary(summary: &foldshift_core::batch::BatchSummary) {
    println!("\n====================================================");
    println!("  Batch Execution Summary");
    println!("====================================================");
    println!("  Total items:  {}", summary.total);
    println!("  Succeeded:    {}", summary.succeeded);
    println!("  Failed:       {}", summary.failed);
    println!("  Cancelled:    {}", summary.cancelled);
    println!("  Elapsed time: {}ms", summary.duration_ms);
    println!("====================================================");
}

fn pdf_inspect(file: PathBuf, json_output: bool) -> Result<()> {
    let inspection = foldshift_pdf::inspect(&file)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&inspection)?);
    } else {
        println!("File    {}", inspection.path.display());
        println!("Type    {}", inspection.format);
        println!("Size    {} bytes", inspection.bytes);
        for (key, value) in inspection.properties {
            println!("{:<12}{}", title_case(&key), plain_json(value));
        }
    }
    Ok(())
}

fn pdf_merge(inputs: Vec<PathBuf>, output: PathBuf, force: bool) -> Result<()> {
    foldshift_pdf::merge(&inputs, &output, foldshift_pdf::MergeOptions { force })?;
    println!("Merged {} files -> {}", inputs.len(), output.display());
    Ok(())
}

fn pdf_extract(input: PathBuf, output: PathBuf, pages: String, force: bool) -> Result<()> {
    foldshift_pdf::extract(
        &input,
        &output,
        foldshift_pdf::ExtractOptions {
            force,
            pages: pages.clone(),
        },
    )?;
    println!(
        "Extracted pages {} from {} -> {}",
        pages,
        input.display(),
        output.display()
    );
    Ok(())
}

fn audio_inspect(file: PathBuf, json_output: bool) -> Result<()> {
    let inspection = foldshift_media::inspect(&file)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&inspection)?);
    } else {
        println!("File    {}", inspection.path.display());
        println!("Type    {}", inspection.format);
        println!("Size    {} bytes", inspection.bytes);
        for (key, value) in inspection.properties {
            println!("{:<14}{}", title_case(&key), plain_json(value));
        }
    }
    Ok(())
}

fn audio_convert(args: AudioConvertArgs) -> Result<()> {
    foldshift_media::convert(
        &args.input,
        &args.output,
        foldshift_media::AudioConvertOptions {
            bitrate: args.bitrate,
            sample_rate: args.sample_rate,
            channels: args.channels,
            force: args.force,
        },
    )?;
    println!(
        "Converted {} -> {}",
        args.input.display(),
        args.output.display()
    );
    Ok(())
}

fn video_inspect(file: PathBuf, json_output: bool) -> Result<()> {
    let inspection = foldshift_media::inspect(&file)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&inspection)?);
    } else {
        println!("File    {}", inspection.path.display());
        println!("Type    {}", inspection.format);
        println!("Size    {} bytes", inspection.bytes);
        for (key, value) in inspection.properties {
            println!("{:<14}{}", title_case(&key), plain_json(value));
        }
    }
    Ok(())
}

fn parse_resolution(res: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = res.split('x').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "invalid resolution format: '{}', expected WIDTHxHEIGHT (e.g. '1920x1080')",
            res
        );
    }
    let w = parts[0]
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("invalid width in resolution '{}'", res))?;
    let h = parts[1]
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("invalid height in resolution '{}'", res))?;
    if w == 0 || h == 0 {
        anyhow::bail!("resolution dimensions must be greater than zero");
    }
    Ok((w, h))
}

fn video_convert(args: VideoConvertArgs) -> Result<()> {
    let resolution = if let Some(r) = &args.resolution {
        Some(parse_resolution(r)?)
    } else {
        match (args.width, args.height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    };
    foldshift_media::convert_video(
        &args.input,
        &args.output,
        foldshift_media::VideoConvertOptions {
            resolution,
            width: args.width,
            height: args.height,
            fps: args.fps,
            crf: args.crf,
            video_bitrate: args.video_bitrate,
            audio_bitrate: args.audio_bitrate,
            mute_audio: args.mute_audio,
            video_codec: args.video_codec,
            force: args.force,
        },
    )?;
    println!(
        "Converted {} -> {}",
        args.input.display(),
        args.output.display()
    );
    Ok(())
}

fn video_extract_audio(args: VideoExtractAudioArgs) -> Result<()> {
    foldshift_media::extract_audio(&args.input, &args.output, args.force)?;
    println!(
        "Extracted audio {} -> {}",
        args.input.display(),
        args.output.display()
    );
    Ok(())
}

fn batch_video_cmd(args: BatchVideoArgs) -> Result<()> {
    let cancel_token = foldshift_core::batch::CancellationToken::new();
    let handler_token = cancel_token.clone();
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let interrupt_clone = interrupt_count.clone();

    let _ = ctrlc::set_handler(move || {
        let prev = interrupt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 0 {
            eprintln!(
                "\nCancellation requested (Ctrl+C). Safely stopping video batch after current item..."
            );
            handler_token.cancel();
        } else {
            eprintln!("\nSecond interrupt received. Terminating immediately.");
            std::process::exit(130);
        }
    });

    let resolution = if let Some(r) = &args.resolution {
        Some(parse_resolution(r)?)
    } else {
        match (args.width, args.height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    };

    let json_mode = args.json;
    let summary = foldshift_media::batch_convert_video(
        &args.inputs,
        &args.to,
        &args.out_dir,
        args.pattern.as_deref(),
        foldshift_media::VideoConvertOptions {
            resolution,
            width: args.width,
            height: args.height,
            fps: args.fps,
            crf: args.crf,
            video_bitrate: args.video_bitrate,
            audio_bitrate: args.audio_bitrate,
            mute_audio: args.mute_audio,
            video_codec: args.video_codec,
            force: args.force,
        },
        args.concurrency,
        &cancel_token,
        |ev| {
            if !json_mode {
                render_batch_progress(&ev);
            }
        },
    )?;

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        render_batch_summary(&summary);
    }

    if summary.has_failures_or_cancellations() {
        anyhow::bail!(
            "Batch completed with issues: {} succeeded, {} failed, {} cancelled",
            summary.succeeded,
            summary.failed,
            summary.cancelled
        );
    }

    Ok(())
}

fn title_case(value: &str) -> String {
    value
        .split('_')
        .map(|segment| {
            let mut characters = segment.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn plain_json(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value,
        other => other.to_string(),
    }
}
