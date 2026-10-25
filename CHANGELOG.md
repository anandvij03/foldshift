# Changelog

## Unreleased

### Added

- Audio conversion and stream inspection adapter (`foldshift-media`) supporting MP3, WAV, FLAC, OGG, M4A, AAC, WMA, and OPUS.
- Shared media engine pack architecture (`media`) utilizing FFmpeg with bounded subprocess execution (60s timeout, closed stdin, concurrent pipe draining, 10 MB safety cap).
- Safe publication and atomic staging in destination parent directory (`tempfile::NamedTempFile`) with same-file overwrite protection.
- CLI subcommands for audio: `foldshift audio inspect`, `foldshift audio convert`, and `foldshift batch audio` with custom naming pattern resolution, cancellation tokens, and progress reporting.
- Web UI Audio Studio with format selection, bitrate, sample rate, channels configuration, and live inspection metadata reporting via `/api/audio/inspect` and `/api/audio/convert`.
- Foldshift CLI for still-image conversion, proportional resizing, inspection,
  and JSON capability reporting.
- PNG, JPEG, WebP, GIF, BMP, and TIFF support through the `image` library.
- Multi-frame and multi-page guards: animated GIF, APNG, animated WebP, and
  multi-page TIFF are detected and rejected early to avoid silent data loss.
- Early preflight checks for destination format, quality range, collision, and
  resize dimensions before decode, preventing filesystem side effects on invalid requests.
- Single-frame GIF and single-page TIFF support with regression coverage.
- Actionable `FcError::UnsupportedContent` error variant in `foldshift-core`.
- EXIF orientation normalization: pixel geometry is rotated/flipped during decode to produce upright outputs.
- Header-based `inspect` metadata reading: orientation, effective dimensions, and color types are inspected without decoding full pixel buffers.
- Native 1-component grayscale JPEG encoding for 8-bit and 16-bit luma inputs without expanding to RGB.
- Allocation-free inspection of native pixel buffers for transparency, allowing fully opaque alpha channels to convert cleanly to JPEG.
- Output preservation and temporary-file publication through `tempfile`.
- Sequential batch conversion and resizing engine in `foldshift-core` and `foldshift-image` with independent failure isolation.
- Flexible and secure naming pattern resolution (`{stem}`, `{name}`, `{ext}`, `{index}`, `{index:03}`, `{index0}`) with intra-batch collision detection and path traversal rejection.
- Cooperative cancellation tokens (`CancellationToken`) and CLI signal handling (Ctrl+C) ensuring safe termination without destination corruption.
- Real-time item-by-item progress reporting and machine-readable JSON summaries via `foldshift batch convert` and `foldshift batch resize`.
- Tests for conversion, resizing, corrupt input, overwrite behavior, and sequential batch execution.
- CI configuration for Windows, macOS, and Linux; contributor and issue templates.

### Changed

- Replaced silent flattening of animations and multi-page images with explicit
  refusal errors before expensive decode.
- Replaced whole-image pixel decoding in `inspect` with lightweight decoder header queries.
- Replaced full RGBA16 buffer copying during JPEG transparency validation with zero-copy native channel inspection.
- Adopted Foldshift as the project and command name, replacing the LocalConvert
  prototype and its `fc` command.

### Known limitations

- Still images only: animated inputs and multi-page TIFFs are rejected rather
  than converted, preserved, or split into individual frames.
- Embedded ICC color profiles are not converted or preserved (pixels remain in their source color space).
- Quality is JPEG-only; WebP output is lossless; transparent JPEG export fails.
- Video stream operations, desktop UI, and isolated workers are planned.
