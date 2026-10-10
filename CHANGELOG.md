# Changelog

## Unreleased

### Added

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
- Tests for conversion, resizing, corrupt input, and overwrite behavior.
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
- PDF, media, desktop UI, and isolated workers are not implemented.
