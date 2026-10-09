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
- Output preservation and temporary-file publication through `tempfile`.
- Tests for conversion, resizing, corrupt input, and overwrite behavior.
- CI configuration for Windows, macOS, and Linux; contributor and issue templates.

### Changed

- Replaced silent flattening of animations and multi-page images with explicit
  refusal errors before expensive decode.
- Adopted Foldshift as the project and command name, replacing the LocalConvert
  prototype and its `fc` command.

### Known limitations

- Still images only: animated inputs and multi-page TIFFs are rejected rather
  than converted, preserved, or split into individual frames.
- Embedded orientation and color profiles are not applied or preserved.
- Quality is JPEG-only; WebP output is lossless; transparent JPEG export fails.
- PDF, media, desktop UI, and isolated workers are not implemented.
